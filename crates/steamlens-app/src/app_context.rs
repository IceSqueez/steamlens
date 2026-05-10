use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::time::Instant;

use iced::Task;

use crate::cache::{self, CacheHit, GameCacheEntry};
use crate::messaging::MessagingCenter;
use crate::settings::Settings;
use crate::steam_worker::{SteamReply, SteamWorker};

pub struct AnimationState {
    pub skeleton_phase: f32,
}

pub struct AppContext {
    pub worker: Option<SteamWorker>,
    pub worker_rx: Option<mpsc::Receiver<SteamReply>>,
    pub settings: Settings,
    pub settings_dirty_since: Option<Instant>,
    pub messaging: MessagingCenter,
    pub cached_entries: HashMap<u32, GameCacheEntry>,
    pub pending_hit_queue: VecDeque<CacheHit>,
    pub steam_root: std::path::PathBuf,
    pub steamid3: u64,
    pub steam_level: Option<u32>,
    pub steam_running: Option<bool>,
    pub user_profile: Option<steamlens_core::UserProfile>,
    pub profile_avatar_handle: Option<iced::widget::image::Handle>,
    pub no_ach_cache: cache::NoAchievementsCache,
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
