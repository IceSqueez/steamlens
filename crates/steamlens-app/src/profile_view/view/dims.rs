use iced::Color;

use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::ui::theme::{AppTheme, palette};

pub(super) const CARD_GAP: f32 = 12.0;
pub(super) const MIN_GAP: f32 = 12.0;

pub(super) const CARD_NAME_TEXT_HEIGHT: f32 = 12.0;
pub(super) const CARD_COUNTER_TEXT_SIZE: f32 = 11.0;
pub(super) const CARD_PILL_HEIGHT: f32 = 18.0;
pub(super) const CARD_PROGRESS_BAR_HEIGHT: f32 = 8.0;
pub(super) const CARD_PROGRESS_BAR_INSET: f32 = 16.0;
pub(super) const SKELETON_COUNTER_PILL_WIDTH_RATIO: f32 = 0.18;
pub(super) const SKELETON_GENRE_PILL_WIDTH_RATIO: f32 = 0.28;

pub(super) const CARD_HORIZONTAL_PADDING: f32 = 8.0;
pub(super) const CARD_NAME_ROW_HEIGHT: f32 = 24.0;
pub(super) const CARD_NAME_ROW_PAD_TOP: f32 = 4.0;
pub(super) const CARD_TAGS_ROW_HEIGHT: f32 = 32.0;
pub(super) const CARD_TAGS_ROW_PAD_TOP: f32 = 3.0;
pub(super) const CARD_TAGS_ROW_PAD_BOTTOM: f32 = 8.0;

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

pub(super) fn completion_tier_color(pct: f32, theme: AppTheme) -> Option<Color> {
    let p = palette(theme);
    if pct >= 100.0 {
        Some(p.rarity_legendary)
    } else if pct >= 90.0 {
        Some(p.rarity_mythical)
    } else if pct >= 75.0 {
        Some(p.rarity_rare)
    } else if pct >= 50.0 {
        Some(p.rarity_uncommon)
    } else if pct >= 25.0 {
        Some(p.rarity_common)
    } else {
        None
    }
}

pub(super) fn rarity_color_for_tier(tier: RarityTier, theme: AppTheme) -> Color {
    let p = palette(theme);
    match tier {
        RarityTier::Common => p.rarity_common,
        RarityTier::Uncommon => p.rarity_uncommon,
        RarityTier::Rare => p.rarity_rare,
        RarityTier::Mythical => p.rarity_mythical,
        RarityTier::Legendary => p.rarity_legendary,
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
