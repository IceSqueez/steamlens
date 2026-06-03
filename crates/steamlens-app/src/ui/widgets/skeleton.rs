use iced::widget::{Space, container};
use iced::{Background, Border, Element, Length, Radians, gradient};

use crate::ui::theme::{palette, theme_from_iced};

#[cfg(test)]
use crate::ui::theme::AppTheme;

pub const SKELETON_DEFAULT_RADIUS: f32 = 4.0;

/// `phase` in `[0.0, 1.0)` cycles a bright band left-to-right across the box.
pub fn skeleton_box<'a, M: 'a>(width: f32, height: f32, radius: f32, phase: f32) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Gradient(gradient::Gradient::Linear(
                    build_shimmer_gradient(phase, p.surface, p.hover),
                ))),
                border: Border {
                    radius: radius.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn build_shimmer_gradient(phase: f32, base: iced::Color, shine: iced::Color) -> gradient::Linear {
    let angle = Radians(std::f32::consts::FRAC_PI_2);
    let band_half_width = 0.20f32;

    let lo = phase - band_half_width;
    let hi = phase + band_half_width;

    let mut stops: Vec<(f32, iced::Color)> = Vec::with_capacity(8);
    stops.push((0.0, base));

    if lo < 0.0 {
        let wrapped_lo = lo + 1.0;
        stops.push((wrapped_lo.max(0.001), base));
        stops.push((1.0f32.min(wrapped_lo + 0.001), shine));
        stops.push((1.0, shine));
    } else {
        stops.push(((lo - 0.001).max(0.001), base));
    }

    if hi > 1.0 {
        stops.push((1.0f32.min(phase), shine));
        let wrapped_hi = hi - 1.0;
        stops.push((0.0f32.max(wrapped_hi - 0.001), shine));
        stops.push(((wrapped_hi + 0.001).min(1.0), base));
    } else {
        stops.push((lo.clamp(0.001, 0.999), base));
        stops.push((phase.clamp(0.001, 0.999), shine));
        stops.push((hi.clamp(0.001, 0.999), base));
    }

    stops.push((1.0, base));

    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    stops.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-5);
    stops.truncate(8);

    let mut grad = gradient::Linear::new(angle);
    for (offset, color) in stops {
        grad = grad.add_stop(offset, color);
    }
    grad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_phase_wraps_at_one() {
        let mut phase = 0.99f32;
        phase = (phase + 0.02) % 1.0;
        assert!(phase < 1.0);
        assert!(phase >= 0.0);
    }

    #[test]
    fn skeleton_phase_stays_in_range_after_many_ticks() {
        let mut phase = 0.0f32;
        for _ in 0..1000 {
            phase = (phase + 0.02) % 1.0;
            assert!(phase >= 0.0, "phase went negative: {phase}");
            assert!(phase < 1.0, "phase exceeded 1.0: {phase}");
        }
    }

    #[test]
    fn build_shimmer_gradient_mid_phase_has_stops_in_range() {
        let p = palette(AppTheme::Dark);
        let grad = build_shimmer_gradient(0.5, p.surface, p.hover);
        for stop in grad.stops.into_iter().flatten() {
            assert!(
                (0.0..=1.0).contains(&stop.offset),
                "stop offset out of range: {}",
                stop.offset
            );
        }
    }

    #[test]
    fn build_shimmer_gradient_low_phase_wraps_correctly() {
        let p = palette(AppTheme::Dark);
        let grad = build_shimmer_gradient(0.05, p.surface, p.hover);
        for stop in grad.stops.into_iter().flatten() {
            assert!(
                (0.0..=1.0).contains(&stop.offset),
                "stop offset out of range at low phase: {}",
                stop.offset
            );
        }
    }

    #[test]
    fn build_shimmer_gradient_high_phase_wraps_correctly() {
        let p = palette(AppTheme::Dark);
        let grad = build_shimmer_gradient(0.95, p.surface, p.hover);
        for stop in grad.stops.into_iter().flatten() {
            assert!(
                (0.0..=1.0).contains(&stop.offset),
                "stop offset out of range at high phase: {}",
                stop.offset
            );
        }
    }

    #[test]
    fn skeleton_box_constructs_without_panic() {
        let w = 120.0_f32;
        let h = 45.0_f32;
        let phase = 0.5_f32;
        let _el: iced::Element<'_, ()> = skeleton_box(w, h, SKELETON_DEFAULT_RADIUS, phase);
    }

    #[test]
    fn shimmer_gradient_stops_in_range_for_full_phase_sweep() {
        let p = palette(AppTheme::Dark);
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let grad = build_shimmer_gradient(phase, p.surface, p.hover);
            for stop in grad.stops.into_iter().flatten() {
                assert!(
                    (0.0..=1.0).contains(&stop.offset),
                    "stop offset {} out of range at phase {phase}",
                    stop.offset,
                );
            }
        }
    }

    #[test]
    fn skeleton_constructs_without_panic_in_each_theme() {
        for theme in [AppTheme::Dark, AppTheme::Light] {
            let p = palette(theme);
            let _grad = build_shimmer_gradient(0.5, p.surface, p.hover);
        }
    }
}
