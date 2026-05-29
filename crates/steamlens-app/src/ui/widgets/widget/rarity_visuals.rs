use iced::widget::{container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::game_view::types::RarityTier;
use crate::ui::theme::{AppTheme, palette, theme_from_iced};

pub(crate) fn short_rarity_label_str(label: &str) -> &'static str {
    match label {
        "COMMON" => "COM",
        "UNCOMMON" => "UNC",
        "RARE" => "RARE",
        "MYTHICAL" => "MYTH",
        "LEGENDARY" => "LEG",
        _ => "",
    }
}

pub fn rarity_color(tier: RarityTier, theme: AppTheme) -> Color {
    let p = palette(theme);
    match tier {
        RarityTier::Common => p.rarity_common,
        RarityTier::Uncommon => p.rarity_uncommon,
        RarityTier::Rare => p.rarity_rare,
        RarityTier::Mythical => p.rarity_mythical,
        RarityTier::Legendary => p.rarity_legendary,
    }
}

pub fn rarity_label(tier: RarityTier) -> &'static str {
    match tier {
        RarityTier::Common => "COMMON",
        RarityTier::Uncommon => "UNCOMMON",
        RarityTier::Rare => "RARE",
        RarityTier::Mythical => "MYTHICAL",
        RarityTier::Legendary => "LEGENDARY",
    }
}

pub fn tick_lit_at(unlocked_pct: f32, threshold: u8) -> bool {
    unlocked_pct > 0.0 && unlocked_pct >= threshold as f32
}

pub(crate) fn tick_tier_color(threshold: u8, theme: AppTheme) -> Option<Color> {
    let p = palette(theme);
    match threshold {
        0 => Some(p.rarity_common),
        25 => Some(p.rarity_uncommon),
        50 => Some(p.rarity_rare),
        75 => Some(p.rarity_mythical),
        100 => Some(p.rarity_legendary),
        _ => None,
    }
}

pub fn tick_marks<'a, M: 'a + Clone>(unlocked_pct: f32, theme: AppTheme) -> Element<'a, M> {
    const THRESHOLDS: [u8; 5] = [0, 25, 50, 75, 100];

    let mut ticks_row: iced::widget::Row<'a, M> = row![].spacing(0);

    for (i, threshold) in THRESHOLDS.iter().enumerate() {
        let lit = tick_lit_at(unlocked_pct, *threshold);
        let lit_color_opt = tick_tier_color(*threshold, theme);

        let dot = container(iced::widget::Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |t: &iced::Theme| {
                let p = palette(theme_from_iced(t));
                let color = if lit {
                    lit_color_opt.unwrap_or(p.text_muted)
                } else {
                    p.text_muted
                };
                container::Style {
                    background: Some(Background::Color(color)),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            });

        let label = text(format!("{threshold}%"))
            .size(14)
            .style(move |t: &iced::Theme| {
                let p = palette(theme_from_iced(t));
                let color = if lit {
                    lit_color_opt.unwrap_or(p.text_muted)
                } else {
                    p.text_muted
                };
                iced::widget::text::Style { color: Some(color) }
            });
        let tick_unit = row![dot, label].spacing(3).align_y(Alignment::Center);

        let tick_pct = *threshold as f32;
        let fill_before = if i == 0 {
            tick_pct - 0.5
        } else {
            tick_pct - THRESHOLDS[i - 1] as f32 - 0.5
        };
        let fill_before = fill_before.max(0.0) as u16;

        if fill_before > 0 {
            ticks_row = ticks_row.push(
                iced::widget::Space::new()
                    .width(Length::FillPortion(fill_before))
                    .height(Length::Fixed(20.0)),
            );
        }
        ticks_row = ticks_row.push(tick_unit);
    }

    ticks_row
        .width(Length::Fill)
        .height(Length::Fixed(20.0))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_tier_color_thresholds() {
        use crate::ui::theme::{AppTheme, palette};
        let p = palette(AppTheme::Dark);
        assert_eq!(tick_tier_color(0, AppTheme::Dark), Some(p.rarity_common));
        assert_eq!(tick_tier_color(25, AppTheme::Dark), Some(p.rarity_uncommon));
        assert_eq!(tick_tier_color(50, AppTheme::Dark), Some(p.rarity_rare));
        assert_eq!(tick_tier_color(75, AppTheme::Dark), Some(p.rarity_mythical));
        assert_eq!(
            tick_tier_color(100, AppTheme::Dark),
            Some(p.rarity_legendary)
        );
        assert_eq!(tick_tier_color(42, AppTheme::Dark), None);
    }
}
