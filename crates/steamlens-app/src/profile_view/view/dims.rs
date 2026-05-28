use iced::Color;

use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::ui::widgets::widget::{
    C_RARITY_COMMON, C_RARITY_LEGENDARY, C_RARITY_MYTHICAL, C_RARITY_RARE, C_RARITY_UNCOMMON,
};

pub(super) const CARD_GAP: f32 = 12.0;
pub(super) const MIN_GAP: f32 = 12.0;

pub(super) const CARD_NAME_TEXT_HEIGHT: f32 = 12.0;
pub(super) const CARD_COUNTER_TEXT_SIZE: f32 = 11.0;
pub(super) const CARD_PILL_HEIGHT: f32 = 18.0;
pub(super) const CARD_PROGRESS_BAR_HEIGHT: f32 = 8.0;
pub(super) const CARD_PROGRESS_BAR_INSET: f32 = 16.0;
pub(super) const SKEL_COUNTER_PILL_WIDTH_RATIO: f32 = 0.18;
pub(super) const SKEL_GENRE_PILL_WIDTH_RATIO: f32 = 0.28;

pub(super) const CARD_H_PAD: f32 = 8.0;
pub(super) const CARD_NAME_ROW_HEIGHT: f32 = 24.0;
pub(super) const CARD_NAME_ROW_PAD_TOP: f32 = 4.0;
pub(super) const CARD_TAGS_ROW_HEIGHT: f32 = 32.0;
pub(super) const CARD_TAGS_ROW_PAD_TOP: f32 = 3.0;
pub(super) const CARD_TAGS_ROW_PAD_BOTTOM: f32 = 8.0;
pub(super) fn compute_grid(viewport: f32, card_w: f32, min_gap: f32) -> (usize, f32) {
    let cols_max = ((viewport + min_gap) / (card_w + min_gap)).floor().max(1.0) as usize;

    let mut cols = cols_max;
    loop {
        let total_card_width = cols as f32 * card_w;
        let remainder = (viewport - total_card_width).max(0.0);
        let gap = remainder / (cols as f32 + 1.0);
        if gap >= min_gap || cols == 1 {
            let clamped_gap = gap.max(0.0);
            return (cols, clamped_gap);
        }
        cols -= 1;
    }
}

pub(super) fn fit_contain(natural_w: f32, natural_h: f32, max_w: f32, max_h: f32) -> (f32, f32) {
    if natural_w <= 0.0 || natural_h <= 0.0 {
        return (max_w, max_h);
    }
    let scale = (max_w / natural_w).min(max_h / natural_h);
    (natural_w * scale, natural_h * scale)
}

pub(super) fn capsule_dims(size: CapsuleSize) -> (f32, f32) {
    match size {
        CapsuleSize::Small => (120.0, 45.0),
        CapsuleSize::Medium => (231.0, 87.0),
        CapsuleSize::Large => (460.0, 215.0),
        CapsuleSize::Portrait => (160.0, 240.0),
    }
}

pub(super) fn card_width(size: CapsuleSize) -> f32 {
    let (capsule_w, _) = capsule_dims(size);
    capsule_w + 16.0
}

pub(super) fn total_card_height(capsule_h: f32) -> f32 {
    capsule_h + 8.0 + 9.0 + 24.0 + 8.0 + 8.0 + 32.0 + 8.0
}
pub(super) fn completion_tier_color(pct: f32) -> Option<Color> {
    if pct >= 100.0 {
        Some(C_RARITY_LEGENDARY)
    } else if pct >= 90.0 {
        Some(C_RARITY_MYTHICAL)
    } else if pct >= 75.0 {
        Some(C_RARITY_RARE)
    } else if pct >= 50.0 {
        Some(C_RARITY_UNCOMMON)
    } else if pct >= 25.0 {
        Some(C_RARITY_COMMON)
    } else {
        None
    }
}

pub(super) fn rarity_color_for_tier(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_RARITY_COMMON,
        RarityTier::Uncommon => C_RARITY_UNCOMMON,
        RarityTier::Rare => C_RARITY_RARE,
        RarityTier::Mythical => C_RARITY_MYTHICAL,
        RarityTier::Legendary => C_RARITY_LEGENDARY,
    }
}

pub(super) fn rarity_label(tier: RarityTier) -> &'static str {
    match tier {
        RarityTier::Common => "Common",
        RarityTier::Uncommon => "Uncommon",
        RarityTier::Rare => "Rare",
        RarityTier::Mythical => "Mythical",
        RarityTier::Legendary => "Legendary",
    }
}
