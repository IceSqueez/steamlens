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

const VIRTUAL_BUFFER_ROWS: usize = 2;

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
        let row_h = card_h + row_spacing;
        let total_rows = items.len().div_ceil(cols);

        let first_row = if scroll_y > 0.0 {
            ((scroll_y / row_h).floor() as usize).saturating_sub(VIRTUAL_BUFFER_ROWS)
        } else {
            0
        };

        let viewport_rows = ((size.height / row_h).ceil() as usize).max(1);
        let last_row =
            (first_row + viewport_rows + VIRTUAL_BUFFER_ROWS * 2).min(total_rows.saturating_sub(1));

        let top_pad = first_row as f32 * row_h + padding_top;
        let bottom_rows = total_rows.saturating_sub(last_row + 1);
        let bottom_pad = bottom_rows as f32 * row_h + padding_bottom;

        let mut rows_col: Column<'_, M> = column![].spacing(row_spacing);

        rows_col = rows_col.push(Space::new().height(Length::Fixed(top_pad)));

        let render_start = first_row * cols;
        let render_end = ((last_row + 1) * cols).min(items.len());

        for chunk in items[render_start..render_end].chunks(cols) {
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
        }

        rows_col = rows_col.push(Space::new().height(Length::Fixed(bottom_pad)));

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
    use super::compute_grid;

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
}
