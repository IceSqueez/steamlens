use iced::widget::{Space, container};
use iced::{Background, Border, Element, Length, Radians, gradient};

use crate::ui::theme::{AppTheme, palette};

#[allow(dead_code)]
pub fn skeleton<'a, M: 'a>(width: f32, height: f32, phase: f32, theme: AppTheme) -> Element<'a, M> {
    let p = palette(theme);
    let gradient = build_shimmer_gradient(phase, p.surface, p.hover);

    container(Space::new())
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
            let _el: iced::Element<'_, ()> = skeleton(120.0, 45.0, 0.5, theme);
        }
    }
}
