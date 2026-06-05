use iced::widget::image::Handle as ImageHandle;

use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;

use super::filters::{GameStatusFilter, LibrarySort};

#[derive(Clone)]
pub enum ProfileViewMessage {
    ScanComplete(Vec<steamlens_core::GameSummary>),
    ScanFailed {
        app_id: u32,
        reason: String,
    },
    SearchChanged(String),
    SearchDebounceElapsed(u64),
    SortChanged(LibrarySort),
    CapsuleSizeChanged(CapsuleSize),
    CapsuleLoaded {
        app_id: u32,
        size: CapsuleSize,
        handle: ImageHandle,
        width: u32,
        height: u32,
    },
    CapsuleFailed {
        app_id: u32,
        size: CapsuleSize,
    },
    GameSelected(u32),
    SingleScanRetryRequested(u32),
    StatusFilterChanged(GameStatusFilter),
    GenreFilterToggled(String),
    GenreFilterCleared,
    ProgressFetched {
        app_id: u32,
        earned: u32,
        total: u32,
    },
    ProgressScanDone(u64),
    CardHoverEntered(u32),
    CardHoverExited(u32),
    FailedScansRetryRequested,
    BarSliceHoverEntered(RarityTier),
    BarSliceHoverExited,
    CardTierHovered {
        app_id: u32,
        tier: Option<RarityTier>,
    },
    GamePinToggleRequested(u32),
    GameOpenRequested(u32),
    ProgressResultReceived(Box<crate::progress_scan::ProgressResult>),
    GridScrolled(f32),
}

#[derive(Debug, Clone)]
pub enum ProfileEvent {
    None,
    OpenGame(u32),
    ToggleGamePin(u32),
    DrainedProgress {
        cache_entries: Vec<crate::cache::GameCacheEntry>,
        summary_entries: Vec<crate::cache::types::GameSummaryCache>,
        no_ach_entries: Vec<(u32, u32)>,
    },
}

impl std::fmt::Debug for ProfileViewMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileViewMessage::ScanComplete(v) => {
                write!(f, "ScanComplete({} enumerated)", v.len())
            }
            ProfileViewMessage::ScanFailed { app_id, reason } => {
                write!(f, "ScanFailed({{ app_id: {app_id}, reason: {reason:?} }})")
            }
            ProfileViewMessage::SearchChanged(s) => write!(f, "SearchChanged({s:?})"),
            ProfileViewMessage::SearchDebounceElapsed(generation) => {
                write!(f, "SearchDebounceElapsed(gen={generation})")
            }
            ProfileViewMessage::SortChanged(s) => write!(f, "SortChanged({s:?})"),
            ProfileViewMessage::CapsuleSizeChanged(s) => write!(f, "CapsuleSizeChanged({s})"),
            ProfileViewMessage::CapsuleLoaded {
                app_id,
                size,
                width,
                height,
                ..
            } => write!(f, "CapsuleLoaded(app={app_id}, {size}, {width}x{height})"),
            ProfileViewMessage::CapsuleFailed { app_id, size } => {
                write!(f, "CapsuleFailed(app={app_id}, {size})")
            }
            ProfileViewMessage::GameSelected(id) => write!(f, "GameSelected({id})"),
            ProfileViewMessage::ProgressFetched {
                app_id,
                earned,
                total,
            } => write!(f, "ProgressFetched(app={app_id}, {earned}/{total})"),
            ProfileViewMessage::ProgressScanDone(generation) => {
                write!(f, "ProgressScanDone(gen={generation})")
            }
            ProfileViewMessage::CardHoverEntered(id) => write!(f, "CardHoverEntered({id})"),
            ProfileViewMessage::CardHoverExited(id) => write!(f, "CardHoverExited({id})"),
            ProfileViewMessage::FailedScansRetryRequested => {
                write!(f, "FailedScansRetryRequested")
            }
            ProfileViewMessage::BarSliceHoverEntered(t) => {
                write!(f, "BarSliceHoverEntered({t:?})")
            }
            ProfileViewMessage::BarSliceHoverExited => write!(f, "BarSliceHoverExited"),
            ProfileViewMessage::CardTierHovered { app_id, tier } => {
                write!(f, "CardTierHovered(app={app_id}, tier={tier:?})")
            }
            ProfileViewMessage::GamePinToggleRequested(id) => {
                write!(f, "GamePinToggleRequested({id})")
            }
            ProfileViewMessage::GameOpenRequested(id) => write!(f, "GameOpenRequested({id})"),
            ProfileViewMessage::SingleScanRetryRequested(id) => {
                write!(f, "SingleScanRetryRequested({id})")
            }
            ProfileViewMessage::ProgressResultReceived(r) => {
                write!(f, "ProgressResultReceived(app={})", r.app_id)
            }
            ProfileViewMessage::StatusFilterChanged(f2) => write!(f, "StatusFilterChanged({f2:?})"),
            ProfileViewMessage::GenreFilterToggled(g) => write!(f, "GenreFilterToggled({g:?})"),
            ProfileViewMessage::GenreFilterCleared => write!(f, "GenreFilterCleared"),
            ProfileViewMessage::GridScrolled(y) => write!(f, "GridScrolled({y:.1})"),
        }
    }
}
