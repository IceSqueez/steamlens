use iced::widget::{Column, Row, Space, column, container, responsive, row, scrollable};
use iced::{Alignment, Element, Length};

pub struct GridLayout {
    pub card_w: f32,
    pub card_h: f32,
    pub min_gap: f32,
    pub row_spacing: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
}

const VIRTUAL_ROW_BUFFER: usize = 2;

fn visible_row_range(
    scroll_y: f32,
    viewport_h: f32,
    total_rows: usize,
    row_stride: f32,
    padding_top: f32,
    buffer: usize,
) -> (usize, usize) {
    if total_rows == 0 {
        return (0, 0);
    }
    if row_stride <= 0.0 || viewport_h <= 0.0 {
        return (0, total_rows);
    }
    let top = (scroll_y - padding_top).max(0.0);
    let first_visible = (top / row_stride).floor() as usize;
    let bottom = (scroll_y + viewport_h - padding_top).max(0.0);
    let last_visible = (bottom / row_stride).floor() as usize;
    let last = (last_visible + buffer + 1).min(total_rows).max(1);
    let first = first_visible.saturating_sub(buffer).min(last - 1);
    (first, last)
}

pub fn responsive_card_grid<'a, T, M, F>(
    items: Vec<T>,
    layout: GridLayout,
    scroll_id: iced::widget::Id,
    scroll_y: f32,
    on_scroll: impl Fn(f32) -> M + Clone + 'a,
    make_cell: F,
) -> Element<'a, M>
where
    T: 'a,
    M: 'a + Clone,
    F: 'a + Fn(&T) -> Element<'a, M>,
{
    let GridLayout {
        card_w,
        card_h,
        min_gap,
        row_spacing,
        padding_top,
        padding_bottom,
    } = layout;

    let grid = responsive(move |size| {
        let (cols, gap) = compute_grid(size.width, card_w, min_gap);

        if items.is_empty() {
            let on_scroll_clone = on_scroll.clone();
            let empty_col: Column<'_, M> = column![];
            return scrollable(empty_col)
                .id(scroll_id.clone())
                .on_scroll(move |vp| on_scroll_clone(vp.absolute_offset().y))
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        let total_rows = items.len().div_ceil(cols);
        let row_stride = card_h + row_spacing;
        let (first, last) = visible_row_range(
            scroll_y,
            size.height,
            total_rows,
            row_stride,
            padding_top,
            VIRTUAL_ROW_BUFFER,
        );

        let total_content_h = padding_top
            + total_rows as f32 * card_h
            + (total_rows.saturating_sub(1)) as f32 * row_spacing
            + padding_bottom;
        let count = last - first;
        let top_spacer_h = padding_top + first as f32 * row_stride;
        let rendered_block_h = count as f32 * card_h + count.saturating_sub(1) as f32 * row_spacing;
        let bottom_spacer_h = (total_content_h - top_spacer_h - rendered_block_h).max(0.0);

        let mut rows_col: Column<'_, M> = column![].spacing(0);
        rows_col = rows_col.push(Space::new().height(Length::Fixed(top_spacer_h)));

        for (offset, chunk) in items.chunks(cols).skip(first).take(count).enumerate() {
            let mut cells_row: Row<'_, M> =
                row![Space::new().width(Length::Fixed(gap))].align_y(Alignment::Start);
            for item in chunk {
                cells_row = cells_row.push(make_cell(item));
                cells_row = cells_row.push(Space::new().width(Length::Fixed(gap)));
            }
            for _ in 0..(cols - chunk.len()) {
                cells_row = cells_row.push(Space::new().width(Length::Fixed(card_w)));
                cells_row = cells_row.push(Space::new().width(Length::Fixed(gap)));
            }
            rows_col = rows_col.push(cells_row);
            if offset + 1 < count {
                rows_col = rows_col.push(Space::new().height(Length::Fixed(row_spacing)));
            }
        }

        rows_col = rows_col.push(Space::new().height(Length::Fixed(bottom_spacer_h)));

        let on_scroll_clone = on_scroll.clone();
        scrollable(rows_col)
            .id(scroll_id.clone())
            .on_scroll(move |vp| on_scroll_clone(vp.absolute_offset().y))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    });

    container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn compute_grid(viewport: f32, card_w: f32, min_gap: f32) -> (usize, f32) {
    let cols_max = ((viewport + min_gap) / (card_w + min_gap)).floor().max(1.0) as usize;

    let mut cols = cols_max;
    loop {
        let total_card_width = cols as f32 * card_w;
        let remainder = (viewport - total_card_width).max(0.0);
        let gap = remainder / (cols as f32 + 1.0);
        if gap >= min_gap || cols == 1 {
            return (cols, gap.max(0.0));
        }
        cols -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_grid, visible_row_range};

    #[test]
    fn fixed_card_width_with_uniform_gaps() {
        let (cols, gap) = compute_grid(1000.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 40.0).abs() < 0.01, "expected gap=40, got {gap}");
    }

    #[test]
    fn min_gap_floor_kicks_in() {
        let (cols, gap) = compute_grid(1010.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 42.0).abs() < 0.01, "expected gap=42, got {gap}");
    }

    #[test]
    fn single_column_below_card_width() {
        let (cols, gap) = compute_grid(150.0, 200.0, 12.0);
        assert_eq!(cols, 1);
        assert_eq!(gap, 0.0);
    }

    #[test]
    fn exact_fit_no_remainder_falls_back_to_fewer_cols() {
        let (cols, gap) = compute_grid(1000.0, 250.0, 12.0);
        assert_eq!(cols, 3);
        let expected_gap = (1000.0 - 3.0 * 250.0) / 4.0;
        assert!(
            (gap - expected_gap).abs() < 0.01,
            "expected gap={expected_gap}, got {gap}"
        );
    }

    #[test]
    fn single_column_gap_is_centered() {
        let (cols, gap) = compute_grid(300.0, 200.0, 12.0);
        assert_eq!(cols, 1);
        let expected_gap = (300.0 - 200.0) / 2.0;
        assert!(
            (gap - expected_gap).abs() < 0.01,
            "expected gap={expected_gap}, got {gap}"
        );
    }

    #[test]
    fn gap_never_negative() {
        let (_cols, gap) = compute_grid(50.0, 200.0, 12.0);
        assert!(gap >= 0.0);
    }

    #[test]
    fn visible_row_range_empty_grid() {
        assert_eq!(visible_row_range(0.0, 500.0, 0, 100.0, 8.0, 2), (0, 0));
    }

    #[test]
    fn visible_row_range_all_rows_fit_viewport() {
        let (first, last) = visible_row_range(0.0, 1000.0, 3, 100.0, 8.0, 2);
        assert_eq!((first, last), (0, 3));
    }

    #[test]
    fn visible_row_range_at_top() {
        let (first, _last) = visible_row_range(0.0, 300.0, 20, 100.0, 8.0, 2);
        assert_eq!(first, 0);
    }

    #[test]
    fn visible_row_range_at_bottom() {
        let (_first, last) = visible_row_range(1800.0, 300.0, 20, 100.0, 8.0, 2);
        assert_eq!(last, 20);
    }

    #[test]
    fn visible_row_range_middle() {
        let (first, last) = visible_row_range(1000.0, 300.0, 20, 100.0, 8.0, 2);
        assert!(first > 0, "expected first>0, got {first}");
        assert!(last < 20, "expected last<20, got {last}");
    }

    #[test]
    fn visible_row_range_buffer_does_not_overshoot_on_overscroll() {
        let (first, last) = visible_row_range(10_000.0, 50.0, 3, 100.0, 0.0, 2);
        assert!(first < last);
        assert!(last <= 3);
    }

    #[test]
    fn visible_row_range_zero_row_stride_falls_back_to_all() {
        assert_eq!(visible_row_range(0.0, 300.0, 5, 0.0, 8.0, 2), (0, 5));
    }

    #[test]
    fn visible_row_range_zero_viewport_falls_back_to_all() {
        assert_eq!(visible_row_range(0.0, 0.0, 5, 100.0, 8.0, 2), (0, 5));
    }

    #[test]
    fn visible_row_range_invariant_holds_across_scroll_positions() {
        let total_rows = 37;
        for step in 0..200 {
            let scroll_y = step as f32 * 37.0;
            let (first, last) = visible_row_range(scroll_y, 400.0, total_rows, 120.0, 8.0, 2);
            assert!(
                first < last,
                "first={first} last={last} scroll_y={scroll_y}"
            );
            assert!(
                last <= total_rows,
                "last={last} total_rows={total_rows} scroll_y={scroll_y}"
            );
        }
    }
}
