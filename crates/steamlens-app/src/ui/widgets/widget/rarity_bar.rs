use std::marker::PhantomData;

use iced::widget::column;
use iced::{Element, Length};

use crate::game_view::types::RarityTier;
use crate::ui::theme::AppTheme;
use crate::ui::widgets::bar::{BarColor, BarSegment, segmented_bar};

use super::format::format_thousands;
use super::rarity_visuals::{rarity_color, rarity_label, tick_marks};
use super::summary::WidgetSummary;

const BAR_HEIGHT: f32 = 16.0;
const BAR_RADIUS: f32 = 6.0;

pub fn rarity_bar<'a, M: 'a + Clone>(summary: WidgetSummary, theme: AppTheme) -> RarityBar<'a, M> {
    RarityBar {
        summary,
        theme,
        hovered: None,
        on_hover: None,
        _phantom: PhantomData,
    }
}

pub struct RarityBar<'a, M> {
    summary: WidgetSummary,
    theme: AppTheme,
    hovered: Option<RarityTier>,
    on_hover: Option<Box<dyn Fn(Option<RarityTier>) -> M + 'a>>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M: 'a + Clone> RarityBar<'a, M> {
    pub fn hovered(mut self, tier: Option<RarityTier>) -> Self {
        self.hovered = tier;
        self
    }

    pub fn on_hover<F>(mut self, f: F) -> Self
    where
        F: Fn(Option<RarityTier>) -> M + 'a,
    {
        self.on_hover = Some(Box::new(f));
        self
    }
}

impl<'a, M: 'a + Clone> From<RarityBar<'a, M>> for Element<'a, M> {
    fn from(bar: RarityBar<'a, M>) -> Self {
        let summary = bar.summary;
        let theme = bar.theme;
        let tier_counts: [(RarityTier, u32); 5] = [
            (RarityTier::Common, summary.common_count),
            (RarityTier::Uncommon, summary.uncommon_count),
            (RarityTier::Rare, summary.rare_count),
            (RarityTier::Mythical, summary.mythical_count),
            (RarityTier::Legendary, summary.legendary_count),
        ];

        let total = summary.achievement_total;
        let total_for_pct = total.max(1);

        let mut segments: Vec<BarSegment> = Vec::new();
        let mut tier_at: Vec<Option<RarityTier>> = Vec::new();
        let mut tooltips: Vec<String> = Vec::new();

        for (tier, count) in tier_counts.iter() {
            if *count == 0 {
                continue;
            }
            let pct = *count as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: *count,
                color: rarity_color(*tier, theme).into(),
            });
            tier_at.push(Some(*tier));
            tooltips.push(format!(
                "{} {} \u{00B7} {:.1}%",
                format_thousands(*count),
                rarity_label(*tier),
                pct
            ));
        }

        let unrated = summary.unrated_unlocked();
        if unrated > 0 {
            let pct = unrated as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: unrated,
                color: BarColor::Accent,
            });
            tier_at.push(None);
            tooltips.push(format!(
                "{} Unrated \u{00B7} {:.1}%",
                format_thousands(unrated),
                pct
            ));
        }

        let locked = summary.locked();
        if locked > 0 {
            let pct = locked as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: locked,
                color: BarColor::Locked,
            });
            tier_at.push(None);
            tooltips.push(format!(
                "{} Locked \u{00B7} {:.1}%",
                format_thousands(locked),
                pct
            ));
        }

        let hovered_idx = bar
            .hovered
            .and_then(|t| tier_at.iter().position(|x| *x == Some(t)));

        let tip_lookup = tooltips.clone();
        let mut bar_builder = segmented_bar(segments, Length::Fill, BAR_HEIGHT)
            .radius(BAR_RADIUS)
            .hovered(hovered_idx)
            .tooltip(move |idx| tip_lookup.get(idx).cloned().unwrap_or_default());

        if let Some(on_hover) = bar.on_hover {
            let tier_lookup = tier_at.clone();
            bar_builder = bar_builder.on_hover(move |idx| {
                let tier = idx.and_then(|i| tier_lookup.get(i).copied().flatten());
                on_hover(tier)
            });
        }

        let bar: Element<'a, M> = bar_builder.into();
        let ticks_layer: Element<'a, M> = tick_marks(summary.unlocked_pct(), theme);

        column![bar, ticks_layer].spacing(4).into()
    }
}
