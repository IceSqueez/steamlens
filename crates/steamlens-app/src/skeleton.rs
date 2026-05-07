#![allow(dead_code)]

use iced::widget::container;
use iced::{Background, Border, Element, Length, Radians, gradient};

use crate::theme::{C_HOVER, C_SURFACE};

/// `phase` in `[0.0, 1.0)` cycles a bright band left-to-right across
/// the box.
pub fn skeleton_box<'a, M: 'a>(width: f32, height: f32, phase: f32) -> Element<'a, M> {
    let gradient = build_shimmer_gradient(phase);

    container(iced::widget::Space::new())
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Gradient(gradient::Gradient::Linear(gradient))),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn build_shimmer_gradient(phase: f32) -> gradient::Linear {
    let angle = Radians(std::f32::consts::FRAC_PI_2);

    let band_half_width = 0.20f32;

    let lo = phase - band_half_width;
    let hi = phase + band_half_width;

    let mut stops: Vec<(f32, iced::Color)> = Vec::with_capacity(8);
    stops.push((0.0, C_SURFACE));

    if lo < 0.0 {
        let wrapped_lo = lo + 1.0;
        stops.push((wrapped_lo.max(0.001), C_SURFACE));
        stops.push((1.0f32.min(wrapped_lo + 0.001), C_HOVER));
        stops.push((1.0, C_HOVER));
    } else {
        stops.push(((lo - 0.001).max(0.001), C_SURFACE));
    }

    if hi > 1.0 {
        stops.push((1.0f32.min(phase), C_HOVER));
        let wrapped_hi = hi - 1.0;
        stops.push((0.0f32.max(wrapped_hi - 0.001), C_HOVER));
        stops.push(((wrapped_hi + 0.001).min(1.0), C_SURFACE));
    } else {
        stops.push((lo.clamp(0.001, 0.999), C_SURFACE));
        stops.push((phase.clamp(0.001, 0.999), C_HOVER));
        stops.push((hi.clamp(0.001, 0.999), C_SURFACE));
    }

    stops.push((1.0, C_SURFACE));

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
        let grad = build_shimmer_gradient(0.5);
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
        let grad = build_shimmer_gradient(0.05);
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
        let grad = build_shimmer_gradient(0.95);
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
        let _el: iced::Element<'_, ()> = skeleton_box(120.0, 45.0, 0.5);
    }
}
