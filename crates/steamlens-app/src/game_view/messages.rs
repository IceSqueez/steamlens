use super::state::SeededGameView;
use super::types::{AchievementFilter, AchievementSort, BulkOp, RarityTier};

#[derive(Debug, Clone)]
pub enum GameViewMessage {
    Noop,
    AchievementToggled(String),
    FilterChanged(AchievementFilter),
    RarityTierToggled(RarityTier),
    HiddenPillToggled,
    RarityFilterCleared,
    AchievementSortChanged(AchievementSort),
    UnlockedAtTopToggled,
    SearchChanged(String),
    StatsSearchChanged(String),
    StatsMaxAll,
    StatsResetAll,
    StatsMaxSingle(String),
    StatsResetSingle(String),
    BulkAction(BulkOp),
    ReloadRequested,
    ApplyClicked,
    ApplyConfirmInputChanged(String),
    ApplyConfirmed,
    ApplyCancelled,
    DiscardChanges,
    RevealHidden(String),
    RequestGoBack,
    AchievementsFullyLoaded,
    CapsuleLoaded {
        app_id: u32,
        size: crate::capsule_cache::CapsuleSize,
        handle: iced::widget::image::Handle,
        width: u32,
        height: u32,
    },
    CapsuleFailed {
        app_id: u32,
        size: crate::capsule_cache::CapsuleSize,
    },
    BarSliceHoverEnter(RarityTier),
    BarSliceHoverExit,
    InvalidateCacheClicked(u32),
    CacheSeeded {
        app_id: u32,
        seeded: Box<SeededGameView>,
    },
    AchievementGridScrolled(f32),
    RetryGlobalPercentages,
}

#[derive(Debug, Clone)]
pub enum GameViewEvent {
    None,
    GoBack,
    AchievementsFullyLoaded { app_id: u32 },
    InvalidateCache { app_id: u32 },
}
