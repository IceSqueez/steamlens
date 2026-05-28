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
    RetrySingleFailedScan(u32),
    StatusFilterChanged(GameStatusFilter),
    GenreFilterToggled(String),
    GenreFilterCleared,
    SpinnerTick(f32),
    ProgressFetched {
        app_id: u32,
        earned: u32,
        total: u32,
    },
    ProgressScanDone,
    LoaderPulseTick,
    CardHoverEnter(u32),
    CardHoverExit(u32),
    #[allow(
        dead_code,
        reason = "re-wired when status_bar gets a Retry-Failed link"
    )]
    RetryFailedScans,
    BarSliceHoverEnter(RarityTier),
    BarSliceHoverExit,
    CardTierHovered {
        app_id: u32,
        tier: Option<RarityTier>,
    },
    RequestToggleGamePin(u32),
    RequestOpenGame(u32),
    DrainProgressResults,
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
            ProfileViewMessage::SpinnerTick(a) => write!(f, "SpinnerTick({a:.1})"),
            ProfileViewMessage::ProgressFetched {
                app_id,
                earned,
                total,
            } => write!(f, "ProgressFetched(app={app_id}, {earned}/{total})"),
            ProfileViewMessage::ProgressScanDone => write!(f, "ProgressScanDone"),
            ProfileViewMessage::LoaderPulseTick => write!(f, "LoaderPulseTick"),
            ProfileViewMessage::CardHoverEnter(id) => write!(f, "CardHoverEnter({id})"),
            ProfileViewMessage::CardHoverExit(id) => write!(f, "CardHoverExit({id})"),
            ProfileViewMessage::RetryFailedScans => write!(f, "RetryFailedScans"),
            ProfileViewMessage::BarSliceHoverEnter(t) => write!(f, "BarSliceHoverEnter({t:?})"),
            ProfileViewMessage::BarSliceHoverExit => write!(f, "BarSliceHoverExit"),
            ProfileViewMessage::CardTierHovered { app_id, tier } => {
                write!(f, "CardTierHovered(app={app_id}, tier={tier:?})")
            }
            ProfileViewMessage::RequestToggleGamePin(id) => write!(f, "RequestToggleGamePin({id})"),
            ProfileViewMessage::RequestOpenGame(id) => write!(f, "RequestOpenGame({id})"),
            ProfileViewMessage::RetrySingleFailedScan(id) => {
                write!(f, "RetrySingleFailedScan({id})")
            }
            ProfileViewMessage::DrainProgressResults => write!(f, "DrainProgressResults"),
            ProfileViewMessage::StatusFilterChanged(f2) => write!(f, "StatusFilterChanged({f2:?})"),
            ProfileViewMessage::GenreFilterToggled(g) => write!(f, "GenreFilterToggled({g:?})"),
            ProfileViewMessage::GenreFilterCleared => write!(f, "GenreFilterCleared"),
        }
    }
}
