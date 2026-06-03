use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Instant, SystemTime};

use steamlens_core::{AppLibraryAssets, SteamAppState};

use crate::cache::{self, CacheHit, GameCacheEntry};
use crate::capsule_cache::CapsuleSize;
use crate::messaging::MessagingCenter;
use crate::profile_view::types::StoredCapsule;
use crate::settings::Settings;
use crate::steam_worker::SteamWorker;

pub struct AnimationState {
    pub last_tick: Instant,
    pub skeleton_phase: f32,
}

impl AnimationState {
    pub fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            skeleton_phase: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ConnectivityState {
    pub steam_running: Option<bool>,
    pub user_logged_in: Option<bool>,
}

#[derive(Default)]
pub struct UserState {
    pub steam_root: std::path::PathBuf,
    pub account_id: u32,
    pub steam_level: Option<u32>,
    pub profile: Option<steamlens_core::UserProfile>,
    pub avatar_handle: Option<iced::widget::image::Handle>,
}

pub struct WorkerState {
    pub current: Option<SteamWorker>,
}

#[derive(Default)]
pub struct GameCacheMemory {
    pub entries: HashMap<u32, GameCacheEntry>,
    pub pending_hits: VecDeque<CacheHit>,
    pub last_recompute_at: Option<Instant>,
}

#[derive(Default)]
pub struct CapsuleStore {
    pub handles: HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub unavailable: HashSet<(u32, CapsuleSize)>,
}

#[derive(Default)]
pub struct SteamSnapshot {
    pub app_state: HashMap<u32, SteamAppState>,
    pub app_state_mtime: Option<SystemTime>,
    pub library_assets: HashMap<u32, AppLibraryAssets>,
}

pub struct AppContext {
    pub worker: WorkerState,
    pub settings: Settings,
    pub settings_dirty_since: Option<Instant>,
    pub messaging: MessagingCenter,
    pub game_cache: GameCacheMemory,
    pub user: UserState,
    pub connectivity: ConnectivityState,
    pub no_ach_cache: cache::NoAchievementsCache,
    pub steam: SteamSnapshot,
    pub capsules: CapsuleStore,
    pub animation: AnimationState,
}

impl AppContext {
    pub fn update_settings(&mut self, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings);
        if self.settings_dirty_since.is_none() {
            self.settings_dirty_since = Some(Instant::now());
        }
    }
}
