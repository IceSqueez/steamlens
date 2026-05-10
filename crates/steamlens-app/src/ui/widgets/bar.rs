use iced::widget::{Space, container, mouse_area, row, tooltip};
use iced::{Background, Border, Color, Element, Length, border};

use super::tooltip_box::tooltip_box;

use crate::ui::theme::{AppTheme, palette};

const DEFAULT_RADIUS: f32 = 4.0;
const DEFAULT_GAP_PX: f32 = 1.0;
const HOVER_BRIGHTEN_FACTOR: f32 = 1.25;

pub fn segmented_bar<'a, M: Clone + 'a>(
    segments: Vec<BarSegment>,
    width: impl Into<Length>,
    height: f32,
) -> SegmentedBar<'a, M> {
    SegmentedBar {
        segments,
        width: width.into(),
        height,
        theme: AppTheme::default(),
        radius: DEFAULT_RADIUS,
        gap_px: DEFAULT_GAP_PX,
        hovered_idx: None,
        on_hover: None,
        tooltip_for: None,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BarSegment {
    pub weight: u32,
    pub color: Color,
}

pub struct SegmentedBar<'a, M> {
    segments: Vec<BarSegment>,
    width: Length,
    height: f32,
    theme: AppTheme,
    radius: f32,
    gap_px: f32,
    hovered_idx: Option<usize>,
    on_hover: Option<Box<dyn Fn(Option<usize>) -> M + 'a>>,
    tooltip_for: Option<Box<dyn Fn(usize) -> String + 'a>>,
}

impl<'a, M: Clone + 'a> SegmentedBar<'a, M> {
    pub fn theme(mut self, theme: AppTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn gap_px(mut self, gap: f32) -> Self {
        self.gap_px = gap;
        self
    }

    pub fn hovered(mut self, idx: Option<usize>) -> Self {
        self.hovered_idx = idx;
        self
    }

    pub fn on_hover<F>(mut self, f: F) -> Self
    where
        F: Fn(Option<usize>) -> M + 'a,
    {
        self.on_hover = Some(Box::new(f));
        self
    }

    pub fn tooltip<F>(mut self, f: F) -> Self
    where
        F: Fn(usize) -> String + 'a,
    {
        self.tooltip_for = Some(Box::new(f));
        self
    }
}

impl<'a, M: Clone + 'a> From<SegmentedBar<'a, M>> for Element<'a, M> {
    fn from(bar: SegmentedBar<'a, M>) -> Self {
        let palette = palette(bar.theme);
        let track_color = palette.hover;
        let radius = bar.radius;
        let height = bar.height;

        let total_weight: u32 = bar.segments.iter().map(|s| s.weight).sum();

        if total_weight == 0 {
            return container(Space::new())
                .width(bar.width)
                .height(Length::Fixed(height))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(track_color)),
                    border: Border {
                        radius: radius.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into();
        }

        let active_count = bar.segments.iter().filter(|s| s.weight > 0).count();
        let mut bar_row: iced::widget::Row<'a, M> = row![].spacing(0);
        let mut placed = 0usize;
        let total_segs = bar.segments.len();

        for (idx, seg) in bar.segments.iter().enumerate() {
            if seg.weight == 0 {
                continue;
            }

            let is_first = placed == 0;
            let is_last = placed + 1 == active_count;
            let needs_gap_before = !is_first && bar.gap_px > 0.0;

            if needs_gap_before {
                bar_row = bar_row.push(
                    container(Space::new())
                        .width(Length::Fixed(bar.gap_px))
                        .height(Length::Fixed(height))
                        .style(move |_: &iced::Theme| container::Style {
                            background: Some(Background::Color(track_color)),
                            ..container::Style::default()
                        }),
                );
            }

            let is_hovered = bar.hovered_idx == Some(idx);
            let effective_color = if is_hovered {
                brighten(seg.color, HOVER_BRIGHTEN_FACTOR)
            } else {
                seg.color
            };

            let seg_radius = border::Radius {
                top_left: if is_first { radius } else { 0.0 },
                bottom_left: if is_first { radius } else { 0.0 },
                top_right: if is_last { radius } else { 0.0 },
                bottom_right: if is_last { radius } else { 0.0 },
            };

            let seg_widget = container(Space::new())
                .width(Length::FillPortion(seg.weight as u16))
                .height(Length::Fixed(height))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(effective_color)),
                    border: Border {
                        radius: seg_radius,
                        ..Border::default()
                    },
                    ..container::Style::default()
                });

            let mut interactive: Element<'a, M> = if let Some(f) = bar.tooltip_for.as_ref() {
                let tip_text = f(idx);
                tooltip_box(seg_widget, tip_text, tooltip::Position::Top)
            } else {
                seg_widget.into()
            };

            if let Some(on_hover) = bar.on_hover.as_ref() {
                let enter_msg = on_hover(Some(idx));
                let exit_msg = on_hover(None);
                interactive = mouse_area(interactive)
                    .on_enter(enter_msg)
                    .on_exit(exit_msg)
                    .into();
            }

            bar_row = bar_row.push(interactive);
            placed += 1;
        }

        let _ = total_segs;

        container(bar_row)
            .width(bar.width)
            .height(Length::Fixed(height))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(track_color)),
                border: Border {
                    radius: radius.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
    }
}

fn brighten(c: Color, factor: f32) -> Color {
    Color {
        r: (c.r * factor).min(1.0),
        g: (c.g * factor).min(1.0),
        b: (c.b * factor).min(1.0),
        a: c.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_segments() -> Vec<BarSegment> {
        vec![
            BarSegment {
                weight: 3,
                color: Color::from_rgb(0.5, 0.5, 0.5),
            },
            BarSegment {
                weight: 2,
                color: Color::from_rgb(0.7, 0.3, 0.9),
            },
        ]
    }

    #[test]
    fn empty_bar_renders_track_only() {
        let _: Element<'_, ()> = segmented_bar::<()>(vec![], Length::Fill, 8.0).into();
    }

    #[test]
    fn bar_with_segments_constructs() {
        let _: Element<'_, ()> =
            segmented_bar::<()>(make_segments(), Length::Fixed(200.0), 8.0).into();
    }

    #[test]
    fn bar_with_all_options_constructs_in_each_theme() {
        for theme in [AppTheme::Dark, AppTheme::Light] {
            let _: Element<'_, ()> = segmented_bar(make_segments(), Length::Fill, 6.0)
                .theme(theme)
                .radius(2.0)
                .gap_px(2.0)
                .hovered(Some(0))
                .on_hover(|_idx| ())
                .tooltip(|idx| format!("slice {idx}"))
                .into();
        }
    }

    #[test]
    fn brighten_clamps_at_one() {
        let c = brighten(Color::from_rgb(0.95, 0.95, 0.95), 1.25);
        assert!(c.r <= 1.0);
        assert!(c.g <= 1.0);
        assert!(c.b <= 1.0);
    }

    #[test]
    fn zero_weight_segments_skipped() {
        let segs = vec![
            BarSegment {
                weight: 0,
                color: Color::WHITE,
            },
            BarSegment {
                weight: 5,
                color: Color::BLACK,
            },
        ];
        let _: Element<'_, ()> = segmented_bar::<()>(segs, Length::Fill, 8.0).into();
    }
}
