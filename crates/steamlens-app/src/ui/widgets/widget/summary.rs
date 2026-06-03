use iced::widget::{column, row, text};
use iced::{Alignment, Element, Length};

use crate::game_view::types::RarityTier;
use crate::ui::theme::{palette, theme_from_iced};

use super::format::format_thousands;

#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetSummary {
    pub earned_total: u32,
    pub achievement_total: u32,
    pub legendary_count: u32,
    pub mythical_count: u32,
    pub rare_count: u32,
    pub uncommon_count: u32,
    pub common_count: u32,
}

impl WidgetSummary {
    pub fn rated_unlocked(&self) -> u32 {
        self.legendary_count
            + self.mythical_count
            + self.rare_count
            + self.uncommon_count
            + self.common_count
    }

    pub fn unrated_unlocked(&self) -> u32 {
        self.earned_total.saturating_sub(self.rated_unlocked())
    }

    pub fn locked(&self) -> u32 {
        self.achievement_total.saturating_sub(self.earned_total)
    }

    pub fn unlocked_pct(&self) -> f32 {
        if self.achievement_total > 0 {
            self.earned_total as f32 / self.achievement_total as f32 * 100.0
        } else {
            0.0
        }
    }

    pub fn pct_to_go(&self) -> f64 {
        if self.achievement_total > 0 {
            self.locked() as f64 / self.achievement_total as f64 * 100.0
        } else {
            0.0
        }
    }

    pub fn add_tier(&mut self, tier: RarityTier, count: u32) {
        match tier {
            RarityTier::Legendary => self.legendary_count += count,
            RarityTier::Mythical => self.mythical_count += count,
            RarityTier::Rare => self.rare_count += count,
            RarityTier::Uncommon => self.uncommon_count += count,
            RarityTier::Common => self.common_count += count,
        }
    }
}

pub fn breakdown_label<'a, M: 'a>() -> Element<'a, M> {
    text("ACHIEVEMENTS BREAKDOWN")
        .size(10)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_muted),
        })
        .into()
}

pub fn earnings_row<'a, M: 'a>(summary: &WidgetSummary) -> Element<'a, M> {
    let earned = summary.earned_total;
    let total = summary.achievement_total;
    let pct = if total > 0 {
        earned as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let earned_text = text(format_thousands(earned))
        .size(20)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_primary),
        });
    let total_text = text(format!("/ {}", format_thousands(total)))
        .size(20)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_dim),
        });
    let pct_text = text(format!("{pct:.1}% unlocked"))
        .size(12)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).accent),
        });

    let counter_row = row![earned_text, total_text]
        .spacing(6)
        .align_y(Alignment::Center);

    column![counter_row, pct_text]
        .spacing(2)
        .align_x(Alignment::End)
        .into()
}

pub fn breakdown_row<'a, M: 'a>(summary: &WidgetSummary) -> Element<'a, M> {
    row![
        breakdown_label::<M>(),
        iced::widget::Space::new().width(Length::Fill),
        earnings_row::<M>(summary),
    ]
    .align_y(Alignment::End)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_derives_locked_and_pct() {
        let summary = WidgetSummary {
            earned_total: 75,
            achievement_total: 100,
            legendary_count: 5,
            mythical_count: 10,
            rare_count: 15,
            uncommon_count: 20,
            common_count: 25,
        };
        assert_eq!(summary.locked(), 25);
        assert_eq!(summary.rated_unlocked(), 75);
        assert_eq!(summary.unrated_unlocked(), 0);
        assert!((summary.unlocked_pct() - 75.0).abs() < 0.01);
        assert!((summary.pct_to_go() - 25.0).abs() < 0.01);
    }

    #[test]
    fn summary_zero_total_safe() {
        let summary = WidgetSummary::default();
        assert_eq!(summary.locked(), 0);
        assert_eq!(summary.unlocked_pct(), 0.0);
        assert_eq!(summary.pct_to_go(), 0.0);
    }
}
