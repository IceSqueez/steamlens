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
