use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::Task;
use tokio::sync::mpsc;

use crate::app_context::{AnimationState, AppContext, ConnectivityState};
use crate::cache;
use crate::messaging::MessagingCenter;
use crate::profile_view::types::ProfileViewState;
use crate::settings::Settings;
use crate::steam_worker::SteamWorker;
use crate::{App, BootStage, Message, Modals, Screen, splash_commands, update_check};

pub(crate) fn boot_with_settings(loaded_settings: Settings) -> (App, Task<Message>) {
    let mut pv_state = ProfileViewState::new();
    pv_state.sort = loaded_settings.library.sort;

    let (reply_tx, reply_rx) = mpsc::unbounded_channel();
    let worker = SteamWorker::spawn(reply_tx.clone());

    let context = AppContext {
        worker: Some(worker),
        worker_reply_tx: reply_tx,
        worker_reply_rx: Arc::new(Mutex::new(Some(reply_rx))),
        settings: loaded_settings,
        settings_dirty_since: None,
        messaging: MessagingCenter::new(),
        cached_entries: HashMap::new(),
        pending_hit_queue: VecDeque::new(),
        last_hit_recompute_at: None,
        steam_root: PathBuf::new(),
        steamid3: 0,
        user_profile: None,
        profile_avatar_handle: None,
        connectivity: ConnectivityState::default(),
        steam_level: None,
        no_ach_cache: cache::load_no_achievements_cache_blocking(),
        steam_state: HashMap::new(),
        steam_state_mtime: None,
        app_assets: HashMap::new(),
        capsule_handles: HashMap::new(),
        capsule_unavailable: HashSet::new(),
        animation: AnimationState::new(),
    };
    tracing::info!(
        "no_ach: cache loaded with {} entries",
        context.no_ach_cache.entries.len()
    );

    let app = App {
        context,
        screen: Screen::ProfileView(Box::new(pv_state)),
        preserved_profile_state: None,
        boot: BootStage::default(),
        modals: Modals::default(),
    };

    let last_steamid3 = app.context.settings.last_user_steamid;

    let mut boot_tasks = vec![
        splash_commands::min_splash_wait(),
        splash_commands::probe_steam_boot(),
        spawn_local_profile_load(),
        spawn_app_assets_load(),
        Task::perform(update_check::check_for_update(), Message::UpdateCheckResult),
    ];

    if let Some(steamid3) = last_steamid3 {
        use crate::cache;
        boot_tasks.push(cache::commands::load_profile_cache(steamid3));
        boot_tasks.push(cache::commands::load_library_cache(steamid3));
    }

    (app, Task::batch(boot_tasks))
}

pub(crate) fn spawn_local_profile_load() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(|| steamlens_core::load_local_profile().ok())
                .await
                .ok()
                .flatten()
        },
        |profile| Message::LocalProfileLoaded(profile.map(Box::new)),
    )
}

pub(crate) fn spawn_app_assets_load() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(steamlens_core::discover_app_assets)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(error = %e, "app_assets load task panicked");
                    HashMap::new()
                })
        },
        Message::AppAssetsLoaded,
    )
}

pub(crate) fn spawn_steam_state_refresh(
    steam_root: std::path::PathBuf,
    steamid3: u64,
    known_mtime: Option<std::time::SystemTime>,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let current_mtime = steamlens_core::read_steam_state_mtime(&steam_root, steamid3);
                if current_mtime.is_some() && current_mtime == known_mtime {
                    return None;
                }
                let (map, mtime) = steamlens_core::read_steam_state(&steam_root, steamid3);
                Some((map, mtime))
            })
            .await
            .ok()
            .flatten()
        },
        Message::SteamStateRefreshed,
    )
}
