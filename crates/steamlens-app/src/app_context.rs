use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use iced::Task;
use steamlens_core::{AppLibraryAssets, SteamAppState};
use tokio::sync::mpsc;

use crate::cache::{self, CacheHit, GameCacheEntry};
use crate::messaging::MessagingCenter;
use crate::settings::Settings;
use crate::steam_worker::{SteamReply, SteamWorker};

pub type SharedWorkerRx = Arc<Mutex<Option<mpsc::UnboundedReceiver<SteamReply>>>>;

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
    /// `None` covers both pre-probe AND "Steam not running" — we can't check login without a live pipe.
    pub user_logged_in: Option<bool>,
}

pub struct AppContext {
    pub worker: Option<SteamWorker>,
    pub worker_reply_tx: mpsc::UnboundedSender<SteamReply>,
    pub worker_reply_rx: SharedWorkerRx,
    pub settings: Settings,
    pub settings_dirty_since: Option<Instant>,
    pub messaging: MessagingCenter,
    pub cached_entries: HashMap<u32, GameCacheEntry>,
    pub pending_hit_queue: VecDeque<CacheHit>,
    pub steam_root: std::path::PathBuf,
    pub steamid3: u64,
    pub steam_level: Option<u32>,
    pub connectivity: ConnectivityState,
    pub user_profile: Option<steamlens_core::UserProfile>,
    pub profile_avatar_handle: Option<iced::widget::image::Handle>,
    pub no_ach_cache: cache::NoAchievementsCache,
    pub steam_state: HashMap<u32, SteamAppState>,
    pub steam_state_mtime: Option<SystemTime>,
    pub app_assets: HashMap<u32, AppLibraryAssets>,
    pub animation: AnimationState,
}

impl AppContext {
    /// Mutates settings via the provided closure and marks them dirty for
    /// deferred persistence. The actual write is performed by the
    /// `SettingsFlushTick` handler after the debounce window expires.
    pub fn update_settings(&mut self, f: impl FnOnce(&mut Settings)) -> Task<crate::Message> {
        f(&mut self.settings);
        if self.settings_dirty_since.is_none() {
            self.settings_dirty_since = Some(Instant::now());
        }
        Task::none()
    }
}
