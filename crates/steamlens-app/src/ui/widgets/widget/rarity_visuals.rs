use iced::widget::{container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::game_view::types::RarityTier;
use crate::ui::theme::{palette, theme_from_iced};

pub const C_RARITY_COMMON: Color = Color::from_rgb(0.314, 0.980, 0.482);
pub const C_RARITY_UNCOMMON: Color = Color::from_rgb(0.545, 0.914, 0.992);
pub const C_RARITY_RARE: Color = Color::from_rgb(0.741, 0.576, 0.976);
pub const C_RARITY_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
pub const C_RARITY_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

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

pub fn rarity_color(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_RARITY_COMMON,
        RarityTier::Uncommon => C_RARITY_UNCOMMON,
        RarityTier::Rare => C_RARITY_RARE,
        RarityTier::Mythical => C_RARITY_MYTHICAL,
        RarityTier::Legendary => C_RARITY_LEGENDARY,
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

pub(crate) fn tick_tier_color(threshold: u8) -> Option<Color> {
    match threshold {
        0 => Some(C_RARITY_COMMON),
        25 => Some(C_RARITY_UNCOMMON),
        50 => Some(C_RARITY_RARE),
        75 => Some(C_RARITY_MYTHICAL),
        100 => Some(C_RARITY_LEGENDARY),
        _ => None,
    }
}

pub fn tick_marks<'a, M: 'a + Clone>(unlocked_pct: f32) -> Element<'a, M> {
    const THRESHOLDS: [u8; 5] = [0, 25, 50, 75, 100];

    let mut ticks_row: iced::widget::Row<'a, M> = row![].spacing(0);

    for (i, threshold) in THRESHOLDS.iter().enumerate() {
        let lit = tick_lit_at(unlocked_pct, *threshold);
        let lit_color_opt = tick_tier_color(*threshold);

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
        assert_eq!(tick_tier_color(0), Some(C_RARITY_COMMON));
        assert_eq!(tick_tier_color(25), Some(C_RARITY_UNCOMMON));
        assert_eq!(tick_tier_color(50), Some(C_RARITY_RARE));
        assert_eq!(tick_tier_color(75), Some(C_RARITY_MYTHICAL));
        assert_eq!(tick_tier_color(100), Some(C_RARITY_LEGENDARY));
        assert_eq!(tick_tier_color(42), None);
    }
}
