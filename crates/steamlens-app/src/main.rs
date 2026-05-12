mod app_context;
mod cache;
mod capsule_cache;
mod capsule_commands;
mod game_view;
mod ipc_pipe;
mod logging;
mod messaging;
mod paths;
mod profile_view;
mod progress_scan;
mod screen;
mod settings;
mod settings_commands;
mod splash_commands;
mod steam_worker;
mod theme;
mod timeouts;
mod ui;
mod worker;
mod worker_subprocess;

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use iced::keyboard;
use iced::widget::{column, container, text};
use iced::{Color, Element, Subscription, Task};

use app_context::{AnimationState, AppContext, ConnectivityState};
use cache::{CachedLibrary, CachedLibraryEntry, CachedProfile, ClassifyResult, GameCacheEntry};
use game_view::{GameViewEvent, GameViewMessage, GameViewState};
use messaging::{BannerSeverity, MessagingCenter, ToastKind};
use profile_view::types::ProfileEvent;
use profile_view::types::{ProfileViewMessage, ProfileViewState};
use settings::Settings;
use steam_worker::{SteamReply, SteamRequest, SteamWorker};
use steamlens_core::{ProbeError, ProbedProfile, STEAMID64_INDIVIDUAL_MIN, UserProfile};

#[derive(Debug)]
enum Screen {
    ProfileView(Box<ProfileViewState>),
    GameView(Box<GameViewState>),
}

#[derive(Debug, Clone)]
enum ProbeFailure {
    SteamNotRunning,
    NotLoggedIn,
    Other(String),
}

impl From<ProbeError> for ProbeFailure {
    fn from(e: ProbeError) -> Self {
        match e {
            ProbeError::SteamNotRunning => ProbeFailure::SteamNotRunning,
            ProbeError::NotLoggedIn => ProbeFailure::NotLoggedIn,
            other => ProbeFailure::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    GoBack,
    ProfileView(ProfileViewMessage),
    GameView(GameViewMessage),
    PollWorker,
    KeyboardEvent(keyboard::Event),
    SplashMinElapsed,
    ProbeResult(Result<ProbedProfile, ProbeFailure>),
    RetrySteamConnect,
    ProfileCacheLoaded(Option<CachedProfile>),
    LibraryCacheLoaded(Option<CachedLibrary>),
    PersistentCacheWritten(&'static str, Result<(), String>),
    SettingsFlushTick,
    SettingsWritten(Result<(), String>),
    ToastTick,
    ToastHovered(u32, bool),
    DismissToast(u32),
    DismissBanner(u32),
    CacheClassified(ClassifyResult),
    DrainHitQueue,
    CacheWritten {
        app_id: u32,
        result: Result<(), String>,
    },
    SkeletonTick,
    FocusSearch,
    NoAchCacheWritten(Result<(), String>),
    GlobalSearchChanged(String),
    GlobalSortChanged(profile_view::types::LibrarySort),
    GlobalCapsuleSizeChanged(capsule_cache::CapsuleSize),
    GlobalToast(String),
    GameSortChanged(game_view::types::AchievementSort),
    PersistGameSummary(u32),
}

impl std::fmt::Debug for GameViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameViewState")
            .field("app_id", &self.app_id)
            .field("phase", &self.phase)
            .finish()
    }
}

struct App {
    context: AppContext,
    screen: Screen,
    splash_min_elapsed: bool,
    library_cache_resolved: bool,
    cache_classified: bool,
    probe_done: bool,
}

fn boot_with_settings(loaded_settings: Settings) -> (App, Task<Message>) {
    let steam_root = std::path::PathBuf::new();
    let profile_result = steamlens_core::load_local_profile();
    let steamid3 = profile_result
        .as_ref()
        .ok()
        .map(|p| p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN))
        .unwrap_or(0);

    let mut pv_state = ProfileViewState::new();
    pv_state.sort = loaded_settings.library.sort;

    let (worker, rx) = SteamWorker::spawn();

    let user_profile = profile_result.ok();
    let profile_avatar_handle = user_profile
        .as_ref()
        .and_then(|p| p.avatar_png_bytes.as_ref())
        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));

    let context = AppContext {
        worker: Some(worker),
        worker_rx: Some(rx),
        settings: loaded_settings,
        settings_dirty_since: None,
        messaging: MessagingCenter::new(),
        cached_entries: HashMap::new(),
        pending_hit_queue: VecDeque::new(),
        steam_root,
        steamid3,
        user_profile,
        profile_avatar_handle,
        connectivity: ConnectivityState::default(),
        steam_level: None,
        no_ach_cache: cache::load_no_achievements_cache_blocking(),
        animation: AnimationState {
            skeleton_phase: 0.0,
        },
    };
    crate::log!(
        "no_ach: cache loaded with {} entries",
        context.no_ach_cache.entries.len()
    );

    let app = App {
        context,
        screen: Screen::ProfileView(Box::new(pv_state)),
        splash_min_elapsed: false,
        library_cache_resolved: false,
        cache_classified: false,
        probe_done: false,
    };

    (
        app,
        Task::batch([
            splash_commands::min_splash_wait(),
            splash_commands::probe_steam_boot(),
        ]),
    )
}

fn drain_worker_replies(app: &mut App) -> Task<Message> {
    let Some(rx) = &app.context.worker_rx else {
        return Task::none();
    };

    let replies: Vec<SteamReply> = rx.try_iter().collect();
    let mut tasks: Vec<Task<Message>> = Vec::new();

    for reply in replies {
        if let SteamReply::ConnectFailed(reason) = &reply {
            crate::log!("worker: connect failed: {reason}");
            if matches!(app.screen, Screen::GameView(_)) {
                go_back_to_profile(app);
            }
            disconnect_worker(app);
            app.context.connectivity.steam_running = Some(false);
            let already_warned = app.context.messaging.banners.iter().any(|b| {
                b.severity == BannerSeverity::Warning && b.body.starts_with("Steam is not running")
            });
            if !already_warned {
                app.context.messaging.push_banner(
                    BannerSeverity::Warning,
                    "Steam is not running \u{2014} reconnect to load achievements",
                    Some(messaging::BannerAction {
                        label: "Reconnect",
                        message: Message::RetrySteamConnect,
                    }),
                    false,
                );
            }
            return Task::none();
        }

        if let SteamReply::Connected { .. } = &reply
            && let Some(w) = &app.context.worker
        {
            w.send(SteamRequest::RequestUserStats);
            w.send(SteamRequest::RequestGlobalPercentages);
        }

        if let SteamReply::ResetDone = &reply
            && let Some(w) = &app.context.worker
        {
            w.send(SteamRequest::RequestUserStats);
        }

        let Screen::GameView(state) = &mut app.screen else {
            continue;
        };

        let t = game_view::handle_steam_reply(state, reply).map(Message::GameView);
        tasks.push(t);
    }

    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

fn disconnect_worker(app: &mut App) {
    if let Some(w) = &app.context.worker {
        w.send(SteamRequest::Disconnect);
    }
    app.context.worker = None;
    app.context.worker_rx = None;
}

fn go_back_to_profile(app: &mut App) {
    disconnect_worker(app);
    if let Screen::GameView(gv_state) = std::mem::replace(
        &mut app.screen,
        Screen::ProfileView(Box::new(ProfileViewState::new())),
    ) {
        app.screen = Screen::ProfileView(gv_state.prev_profile_state);
    }
}

fn open_game_view(app: &mut App, app_id: u32) -> Task<Message> {
    let prev = if let Screen::ProfileView(pv_state) = std::mem::replace(
        &mut app.screen,
        Screen::ProfileView(Box::new(ProfileViewState::new())),
    ) {
        pv_state
    } else {
        Box::new(ProfileViewState::new())
    };

    disconnect_worker(app);

    let (worker, rx) = SteamWorker::spawn();
    worker.send(SteamRequest::ConnectWithApp(app_id));

    let mut state = GameViewState::new(app_id).with_prev_profile(prev);
    state.achievement_sort = app.context.settings.manager.sort;
    state.rarity_tier_set = app
        .context
        .settings
        .manager
        .rarity_tiers
        .iter()
        .copied()
        .collect();
    state.include_hidden = app.context.settings.manager.include_hidden;

    if let Some(cached) = app.context.cached_entries.get(&app_id) {
        seed_game_view_from_cache(&mut state, cached);
    }

    let capsule_task = Task::perform(
        capsule_cache::fetch_capsule(app_id, capsule_cache::CapsuleSize::Portrait),
        move |result| match result {
            Ok((size, pixels)) => {
                let handle = iced::widget::image::Handle::from_rgba(
                    pixels.width,
                    pixels.height,
                    pixels.rgba,
                );
                Message::GameView(GameViewMessage::CapsuleLoaded {
                    app_id,
                    size,
                    handle,
                    width: pixels.width,
                    height: pixels.height,
                })
            }
            Err((size, _)) => Message::GameView(GameViewMessage::CapsuleFailed { app_id, size }),
        },
    );

    app.context.worker = Some(worker);
    app.context.worker_rx = Some(rx);
    app.screen = Screen::GameView(Box::new(state));

    capsule_task
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::GoBack => match &app.screen {
            Screen::GameView(_) => update(app, Message::GameView(GameViewMessage::RequestGoBack)),
            _ => Task::none(),
        },

        Message::ProfileView(msg) => {
            let Screen::ProfileView(pv_state) = &mut app.screen else {
                #[cfg(debug_assertions)]
                crate::log!(
                    "dropped stale ProfileView message: {msg:?} (current screen: not ProfileView)"
                );
                return Task::none();
            };

            let is_scan_complete = matches!(msg, ProfileViewMessage::ScanComplete(_));
            let is_scan_failed = matches!(msg, ProfileViewMessage::ScanFailed { .. });
            let scan_failed_details =
                if let ProfileViewMessage::ScanFailed { app_id, ref reason } = msg {
                    Some((app_id, reason.clone()))
                } else {
                    None
                };
            let enumerated_games = if let ProfileViewMessage::ScanComplete(ref v) = msg {
                Some(v.clone())
            } else {
                None
            };

            let (task, event) = profile_view::update(pv_state, msg, &mut app.context);
            let task = task.map(Message::ProfileView);

            let extra = if is_scan_complete {
                app.library_cache_resolved = true;
                crate::log!("library_cache_resolved = true (ScanComplete)");
                let games = enumerated_games.unwrap_or_default();
                let steam_root = app.context.steam_root.clone();
                let steamid3 = app.context.steamid3;
                let classify_task = cache::commands::classify_games(games, steam_root, steamid3);

                let mut tasks: Vec<Task<Message>> = vec![classify_task, task];

                if let Screen::ProfileView(pv_state) = &mut app.screen {
                    if !pv_state.library_name_map.is_empty() {
                        let name_map = std::mem::take(&mut pv_state.library_name_map);
                        for game in &mut pv_state.games {
                            if let Some(name) = name_map.get(&game.app_id) {
                                game.name = Some(name.clone());
                            }
                        }
                    }
                    if !pv_state.games.is_empty() {
                        let cached = cache::make_cached_library(
                            pv_state
                                .games
                                .iter()
                                .map(|g| CachedLibraryEntry {
                                    app_id: g.app_id,
                                    change_number: g.change_number,
                                    last_played: g.last_played,
                                    name: String::new(),
                                    achievement_count: 0,
                                })
                                .collect(),
                        );
                        tasks.push(cache::commands::write_library_cache(cached));
                    }
                }

                Task::batch(tasks)
            } else if is_scan_failed {
                if let Some((app_id, reason)) = scan_failed_details {
                    let action = messaging::ToastAction {
                        label: "Retry".to_owned(),
                        on_press: crate::Message::ProfileView(
                            crate::profile_view::types::ProfileViewMessage::RetrySingleFailedScan(
                                app_id,
                            ),
                        ),
                    };
                    app.context.messaging.push_toast_with_action(
                        messaging::ToastKind::Error,
                        format!("Failed to load app {app_id}"),
                        Some(reason),
                        action,
                    );
                }
                task
            } else {
                task
            };

            match event {
                ProfileEvent::None => extra,
                ProfileEvent::OpenGame(app_id) => {
                    let open_task = open_game_view(app, app_id);
                    Task::batch([extra, open_task])
                }
                ProfileEvent::ToggleGamePin(id) => {
                    let pin_task = app.context.update_settings(|s| {
                        if let Some(pos) = s.library.pinned.iter().position(|&pid| pid == id) {
                            s.library.pinned.remove(pos);
                        } else {
                            s.library.pinned.push(id);
                        }
                    });
                    Task::batch([extra, pin_task])
                }
                ProfileEvent::DrainedProgress {
                    cache_entries,
                    summary_entries,
                    no_ach_entries,
                } => {
                    let mut tasks: Vec<Task<Message>> = vec![extra];
                    for entry in cache_entries {
                        tasks.push(cache::commands::write_game_cache(entry));
                    }
                    for summary in summary_entries {
                        tasks.push(cache::commands::write_game_summary(summary));
                    }
                    for (app_id, cn) in no_ach_entries {
                        app.context.no_ach_cache.insert(app_id, cn);
                        let snapshot = app.context.no_ach_cache.clone();
                        tasks.push(cache::commands::write_no_ach_cache(snapshot));
                    }
                    Task::batch(tasks)
                }
            }
        }

        Message::CacheClassified(result) => {
            app.cache_classified = true;
            crate::log!("cache_classified = true (CacheClassified)");

            let ClassifyResult {
                hits,
                dirty,
                schema_bumped,
                invalidation_count,
            } = result;

            let hit_count = hits.len();
            app.context.pending_hit_queue.extend(hits);

            if let Screen::ProfileView(pv_state) = &mut app.screen
                && !dirty.is_empty()
            {
                let mut scanner = crate::progress_scan::ProgressScanner::new(dirty);
                pv_state.progress_rx = scanner.take_receiver();
                pv_state.progress_scanner = Some(scanner);
            } else if let Screen::ProfileView(pv_state) = &mut app.screen
                && dirty.is_empty()
                && hit_count > 0
            {
                pv_state.last_scan_completed_at = Some(std::time::Instant::now());
            }

            if invalidation_count > 0 {
                app.context.messaging.push_toast(
                    ToastKind::Info,
                    format!("{invalidation_count} games refreshing (cache invalidated)"),
                    None,
                );
            }

            if schema_bumped > 0 {
                app.context.messaging.push_banner(
                    BannerSeverity::Info,
                    "Cache updated — your library is refreshing in the background.",
                    None,
                    true,
                );
            }
            Task::none()
        }

        Message::DrainHitQueue => {
            const HITS_PER_TICK: usize = 8;
            for _ in 0..HITS_PER_TICK {
                let Some(hit) = app.context.pending_hit_queue.pop_front() else {
                    break;
                };
                let mut entry = hit.entry;
                recompute_tier_breakdown_if_missing(&mut entry);
                if let Screen::ProfileView(pv_state) = &mut app.screen
                    && let Some(game) = pv_state.games.iter_mut().find(|g| g.app_id == hit.app_id)
                {
                    use crate::progress_scan::ProgressData;
                    game.name = Some(entry.name.clone());
                    game.progress = Some(ProgressData {
                        earned: entry.progress.earned,
                        total: entry.progress.total,
                    });
                    game.genre = entry.genre.clone();
                }
                app.context.cached_entries.insert(hit.app_id, entry);
            }
            Task::none()
        }

        Message::CacheWritten { app_id, result } => {
            if let Err(e) = result {
                crate::log!("cache: write failed for app {app_id}: {e}");
            }
            Task::none()
        }

        Message::PersistGameSummary(app_id) => {
            let Screen::GameView(gv_state) = &app.screen else {
                return Task::none();
            };
            if gv_state.app_id != app_id {
                return Task::none();
            }

            let earned = gv_state
                .achievements
                .iter()
                .filter(|a| a.effective_achieved())
                .count() as u32;
            let total = gv_state.achievements.len() as u32;

            let change_number = gv_state
                .prev_profile_state
                .games
                .iter()
                .find(|g| g.app_id == app_id)
                .map(|g| g.change_number)
                .unwrap_or(0);

            let genre = app
                .context
                .cached_entries
                .get(&app_id)
                .and_then(|e| e.genre.clone());

            let name = gv_state.game_name.clone();
            let tier_breakdown = gv_state.tier_breakdown.clone();

            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let summary = crate::cache::types::GameSummaryCache {
                schema_version: crate::cache::types::SUMMARY_SCHEMA_VERSION,
                app_id,
                name,
                cached_change_number: change_number,
                cached_at: now_secs,
                progress: crate::cache::types::CachedProgress { earned, total },
                tier_breakdown,
                genre,
            };

            crate::log!(
                "persist game summary: app_id={app_id} earned={earned} total={total} change_number={change_number}"
            );

            Task::perform(
                async move { crate::cache::store::write_game_summary(&summary).await },
                move |result| Message::CacheWritten {
                    app_id,
                    result: result.map_err(|e| e.to_string()),
                },
            )
        }

        Message::GameView(m) => {
            let Screen::GameView(state) = &mut app.screen else {
                #[cfg(debug_assertions)]
                crate::log!("dropped stale GameView message: {m:?} (current screen: not GameView)");
                return Task::none();
            };

            let (task, event) = game_view::update(state, m, &mut app.context);
            let task = task.map(Message::GameView);

            match event {
                GameViewEvent::None => task,
                GameViewEvent::AchievementsFullyLoaded { app_id } => {
                    let sync_task = update(app, Message::PersistGameSummary(app_id));
                    Task::batch([task, sync_task])
                }
                GameViewEvent::GoBack => {
                    let write_task = {
                        let Screen::GameView(gv_state) = &app.screen else {
                            return task;
                        };
                        let app_id = gv_state.app_id;
                        let mut entry = build_game_view_cache_entry(
                            gv_state,
                            app_id,
                            &app.context.steam_root,
                            app.context.steamid3,
                        );
                        if let Some(existing) = app.context.cached_entries.get(&app_id)
                            && entry.genre.is_none()
                        {
                            entry.genre = existing.genre.clone();
                        }
                        app.context.cached_entries.insert(app_id, entry.clone());
                        cache::commands::write_game_cache(entry)
                    };
                    go_back_to_profile(app);
                    Task::batch([task, write_task])
                }
            }
        }

        Message::PollWorker => drain_worker_replies(app),

        Message::SettingsFlushTick => {
            if let Some(since) = app.context.settings_dirty_since
                && since.elapsed() >= Duration::from_millis(200)
            {
                app.context.settings_dirty_since = None;
                let snapshot = app.context.settings.clone();
                return settings_commands::write_settings(snapshot);
            }
            Task::none()
        }

        Message::SettingsWritten(result) => {
            if let Err(e) = result {
                crate::log!("settings: write error: {e}");
                app.context
                    .messaging
                    .push_toast(ToastKind::Error, "Could not save settings", None);
            }
            Task::none()
        }

        Message::SplashMinElapsed => {
            app.splash_min_elapsed = true;
            Task::none()
        }

        Message::RetrySteamConnect => {
            app.context.connectivity = ConnectivityState::default();
            app.library_cache_resolved = false;
            app.cache_classified = false;
            app.context
                .messaging
                .dismiss_all_banners_by_severity(BannerSeverity::Warning);
            splash_commands::probe_steam_reconnect()
        }

        Message::ProbeResult(result) => {
            app.probe_done = true;
            match result {
                Ok(p) => {
                    app.context.connectivity.steam_running = Some(true);
                    app.context.connectivity.user_logged_in = Some(true);
                    app.context
                        .messaging
                        .dismiss_all_banners_by_severity(BannerSeverity::Warning);
                    let account_name = app
                        .context
                        .user_profile
                        .as_ref()
                        .map(|u| u.account_name.clone())
                        .unwrap_or_default();
                    app.context.steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
                    app.context.profile_avatar_handle = p
                        .avatar_image
                        .as_ref()
                        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));

                    let steam_root_opt: Option<std::path::PathBuf> = None;
                    let cached = cache::make_cached_profile(
                        p.steam_id,
                        p.persona_name.clone(),
                        account_name.clone(),
                        p.avatar_image.clone(),
                        steam_root_opt,
                    );
                    app.context.steam_level = p.steam_level;
                    app.context.user_profile = Some(UserProfile {
                        steam_id: p.steam_id,
                        persona_name: p.persona_name,
                        account_name,
                        avatar_png_bytes: p.avatar_image,
                    });

                    let mut tasks: Vec<Task<Message>> = Vec::new();

                    tasks.push(cache::commands::write_profile_cache(cached));

                    if !p.game_summaries.is_empty() {
                        let pkginfo_count = p.game_summaries.len();
                        crate::log!("packageinfo: {pkginfo_count} games after type-filter");
                        let no_ach = &app.context.no_ach_cache;
                        let cache_entries = no_ach.entries.len();
                        let filtered: Vec<_> = p
                            .game_summaries
                            .into_iter()
                            .filter(|g| !no_ach.is_known_empty(g.app_id, g.change_number))
                            .collect();
                        let total = filtered.len();
                        let dropped = pkginfo_count - total;
                        crate::log!(
                            "no_ach: cache has {cache_entries} entries; filtered {dropped}/{pkginfo_count} pkginfo games; {total} remain for scan"
                        );
                        let _ = total;
                        tasks.push(Task::done(Message::ProfileView(
                            ProfileViewMessage::ScanComplete(filtered),
                        )));
                    } else {
                        tasks.push(cache::commands::load_library_cache());
                    }

                    Task::batch(tasks)
                }
                Err(ProbeFailure::NotLoggedIn) => {
                    app.context.connectivity.steam_running = Some(true);
                    app.context.connectivity.user_logged_in = Some(false);
                    app.context.steam_level = None;
                    crate::log!("probe: connectivity.user_logged_in = false");

                    app.context.messaging.push_banner(
                        BannerSeverity::Warning,
                        "Steam is running but the user is not signed in \u{2014} showing cached data",
                        Some(messaging::BannerAction {
                            label: "Retry",
                            message: Message::RetrySteamConnect,
                        }),
                        false,
                    );

                    Task::batch([
                        cache::commands::load_profile_cache(),
                        cache::commands::load_library_cache(),
                    ])
                }
                Err(ProbeFailure::SteamNotRunning) => {
                    app.context.connectivity.steam_running = Some(false);
                    app.context.connectivity.user_logged_in = None;
                    app.context.steam_level = None;
                    crate::log!("probe: steam_running = false");

                    app.context.messaging.push_banner(
                        BannerSeverity::Warning,
                        "Steam is not running \u{2014} showing cached data",
                        Some(messaging::BannerAction {
                            label: "Retry",
                            message: Message::RetrySteamConnect,
                        }),
                        false,
                    );

                    Task::batch([
                        cache::commands::load_profile_cache(),
                        cache::commands::load_library_cache(),
                    ])
                }
                Err(ProbeFailure::Other(reason)) => {
                    app.context.connectivity.steam_running = None;
                    app.context.connectivity.user_logged_in = None;
                    app.context.steam_level = None;
                    crate::log!("probe failed: {reason}");

                    app.context.messaging.push_banner(
                        BannerSeverity::Warning,
                        "Steam is not running \u{2014} showing cached data",
                        Some(messaging::BannerAction {
                            label: "Retry",
                            message: Message::RetrySteamConnect,
                        }),
                        false,
                    );

                    Task::batch([
                        cache::commands::load_profile_cache(),
                        cache::commands::load_library_cache(),
                    ])
                }
            }
        }

        Message::ProfileCacheLoaded(maybe) => {
            let Some(cached) = maybe else {
                return Task::none();
            };
            if app.context.user_profile.is_some()
                && app.context.connectivity.steam_running != Some(false)
                && app.context.connectivity.user_logged_in != Some(false)
            {
                return Task::none();
            }
            app.context.steamid3 = cached.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
            app.context.steam_level = None;
            app.context.profile_avatar_handle = cached
                .avatar_png_bytes
                .as_ref()
                .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));
            app.context.user_profile = Some(UserProfile {
                steam_id: cached.steam_id,
                persona_name: cached.persona_name,
                account_name: cached.account_name,
                avatar_png_bytes: cached.avatar_png_bytes,
            });
            Task::none()
        }

        Message::LibraryCacheLoaded(maybe) => {
            let games_present = if let Screen::ProfileView(pv) = &app.screen {
                !pv.games.is_empty()
            } else {
                true
            };
            if games_present {
                return Task::none();
            }
            let Some(cached) = maybe else {
                return Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
                    Vec::new(),
                )));
            };
            let summary: Vec<steamlens_core::GameSummary> = cached
                .games
                .iter()
                .map(|e| steamlens_core::GameSummary {
                    app_id: e.app_id,
                    change_number: e.change_number,
                    last_played: e.last_played,
                })
                .collect();
            let name_map: std::collections::HashMap<u32, String> = cached
                .games
                .into_iter()
                .filter(|e| !e.name.is_empty())
                .map(|e| (e.app_id, e.name))
                .collect();
            if let Screen::ProfileView(pv_state) = &mut app.screen {
                pv_state.library_name_map = name_map;
            }
            if app.context.connectivity.steam_running == Some(false) {
                app.context.messaging.push_banner(
                    BannerSeverity::Info,
                    "Showing cached library \u{2014} connect Steam to refresh",
                    None,
                    true,
                );
            }
            app.library_cache_resolved = true;
            crate::log!("library_cache_resolved = true (LibraryCacheLoaded)");
            Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
                summary,
            )))
        }

        Message::NoAchCacheWritten(result) => {
            if let Err(e) = result {
                crate::log!("no_achievements cache: write failed: {e}");
            }
            Task::none()
        }

        Message::PersistentCacheWritten(label, result) => {
            if let Err(e) = result {
                crate::log!("{label} cache: write failed: {e}");
                app.context.messaging.push_banner(
                    BannerSeverity::Error,
                    format!("Cache write failed ({label}): {e}"),
                    None,
                    true,
                );
            }
            Task::none()
        }

        Message::ToastTick => {
            app.context.messaging.tick_toasts();
            Task::none()
        }

        Message::ToastHovered(id, hovered) => {
            app.context.messaging.set_toast_hovered(id, hovered);
            Task::none()
        }

        Message::DismissToast(id) => {
            app.context.messaging.dismiss_toast(id);
            Task::none()
        }

        Message::DismissBanner(id) => {
            app.context.messaging.dismiss_banner(id);
            Task::none()
        }

        Message::SkeletonTick => {
            app.context.animation.skeleton_phase =
                (app.context.animation.skeleton_phase + 0.02) % 1.0;
            Task::none()
        }

        Message::KeyboardEvent(event) => {
            if let keyboard::Event::KeyPressed {
                modifiers,
                key: keyboard::Key::Character(ref c),
                ..
            } = event
            {
                if modifiers.control()
                    && c.as_str() == "s"
                    && let Screen::GameView(state) = &mut app.screen
                    && state.dirty_count() > 0
                    && !state.has_stat_errors()
                {
                    let (task, _event) =
                        game_view::update(state, GameViewMessage::ApplyChanges, &mut app.context);
                    return task.map(Message::GameView);
                }
                if modifiers.command() && c.as_str() == "f" {
                    return Task::done(Message::FocusSearch);
                }
            }
            Task::none()
        }

        Message::FocusSearch => match &app.screen {
            Screen::ProfileView(_) => {
                iced::widget::operation::focus(profile_view::library_search_id())
            }
            Screen::GameView(_) => {
                iced::widget::operation::focus(game_view::achievement_search_id())
            }
        },

        Message::GlobalSearchChanged(query) => match &mut app.screen {
            Screen::ProfileView(state) => {
                let (task, _event) = profile_view::update(
                    state,
                    ProfileViewMessage::SearchChanged(query),
                    &mut app.context,
                );
                task.map(Message::ProfileView)
            }
            Screen::GameView(state) => {
                let (task, event) = game_view::update(
                    state,
                    GameViewMessage::SearchChanged(query),
                    &mut app.context,
                );
                let task = task.map(Message::GameView);
                match event {
                    GameViewEvent::GoBack => {
                        go_back_to_profile(app);
                        task
                    }
                    GameViewEvent::AchievementsFullyLoaded { app_id } => {
                        let sync_task = update(app, Message::PersistGameSummary(app_id));
                        Task::batch([task, sync_task])
                    }
                    GameViewEvent::None => task,
                }
            }
        },

        Message::GlobalSortChanged(sort) => {
            route_to_profile(app, ProfileViewMessage::SortChanged(sort))
        }

        Message::GlobalCapsuleSizeChanged(size) => {
            route_to_profile(app, ProfileViewMessage::CapsuleSizeChanged(size))
        }

        Message::GlobalToast(msg) => {
            app.context.messaging.push_toast(ToastKind::Info, msg, None);
            Task::none()
        }

        Message::GameSortChanged(sort) => {
            let Screen::GameView(state) = &mut app.screen else {
                return Task::none();
            };
            let (task, _event) = game_view::update(
                state,
                GameViewMessage::AchievementSortChanged(sort),
                &mut app.context,
            );
            task.map(Message::GameView)
        }
    }
}

fn route_to_profile(app: &mut App, msg: ProfileViewMessage) -> Task<Message> {
    let Screen::ProfileView(state) = &mut app.screen else {
        return Task::none();
    };
    let (task, _event) = profile_view::update(state, msg, &mut app.context);
    task.map(Message::ProfileView)
}

fn recompute_tier_breakdown_if_missing(entry: &mut GameCacheEntry) {
    use game_view::compute_tier_breakdown;
    use game_view::types::{AchievementData, AchievementRow};

    if !entry.tier_breakdown.is_empty() || entry.achievements.is_empty() {
        return;
    }

    let rows: Vec<AchievementRow> = entry
        .achievements
        .iter()
        .map(|a| {
            let data = AchievementData {
                id: a.api_name.clone(),
                display_name: a.display_name.clone(),
                description: a.description.clone(),
                is_hidden: a.hidden,
                is_achieved: a.earned,
                unlock_time: a.earned_at.map(|t| t as u32),
                permission: 0,
                icon: None,
            };
            let mut row = AchievementRow::from(data);
            row.rarity_percent = a.global_percent.map(|p| p as f32);
            row
        })
        .collect();

    entry.tier_breakdown = compute_tier_breakdown(&rows);
}

fn seed_game_view_from_cache(state: &mut GameViewState, cached: &GameCacheEntry) {
    use game_view::GameViewPhase;
    use game_view::types::{AchievementData, AchievementRow, StatData, StatRow, StatValue};

    if cached.achievements.is_empty() {
        return;
    }

    state.game_name = cached.name.clone();

    state.achievements = cached
        .achievements
        .iter()
        .map(|a| {
            let data = AchievementData {
                id: a.api_name.clone(),
                display_name: a.display_name.clone(),
                description: a.description.clone(),
                is_hidden: a.hidden,
                is_achieved: a.earned,
                unlock_time: a.earned_at.map(|t| t as u32),
                permission: 0,
                icon: None,
            };
            let mut row = AchievementRow::from(data);
            row.appeared = true;
            row.card_opacity = 1.0;
            row.rarity_percent = a.global_percent.map(|p| p as f32);
            row
        })
        .collect();

    state.stats = cached
        .stats
        .iter()
        .map(|s| {
            let value = match s.value {
                cache::types::CachedStatValue::Int(i) => StatValue::Int(i as i32),
                cache::types::CachedStatValue::Float(f) => StatValue::Float(f as f32),
            };
            let data = StatData {
                id: s.api_name.clone(),
                display_name: s.display_name.clone(),
                value,
                original_value: value,
                max_value: None,
                min_value: None,
                default_value: None,
                is_increment_only: false,
                permission: 0,
            };
            StatRow::from(data)
        })
        .collect();

    state.phase = GameViewPhase::Connecting;
    state.reveal_queue.clear();
}

pub(crate) fn build_cache_entry_from_scan(
    scanned: &progress_scan::ScannedGameData,
    app_id: u32,
    entry_name: Option<&str>,
    steam_root: &std::path::Path,
    steamid3: u64,
) -> GameCacheEntry {
    use cache::types::{CachedProgress, CachedStat};
    use game_view::compute_tier_breakdown;
    use game_view::types::{AchievementData, AchievementRow};
    use steamlens_core::{StatValue, read_last_played};

    let stats: Vec<CachedStat> = scanned
        .stats
        .iter()
        .map(|s| {
            let value = match s.value {
                StatValue::Int(i) => cache::types::CachedStatValue::Int(i as i64),
                StatValue::Float(f) => cache::types::CachedStatValue::Float(f as f64),
            };
            CachedStat {
                api_name: s.id.clone(),
                display_name: s.display_name.clone(),
                value,
            }
        })
        .collect();

    let earned = scanned
        .achievements
        .iter()
        .filter(|a| a.is_achieved)
        .count() as u32;
    let total = scanned.achievements.len() as u32;

    let tier_rows: Vec<AchievementRow> = scanned
        .achievements
        .iter()
        .map(|a| {
            let data = AchievementData {
                id: a.id.clone(),
                display_name: String::new(),
                description: String::new(),
                is_hidden: false,
                is_achieved: a.is_achieved,
                unlock_time: None,
                permission: 0,
                icon: None,
            };
            let mut row = AchievementRow::from(data);
            row.rarity_percent = scanned.global_percentages.get(&a.id).copied();
            row
        })
        .collect();
    let tier_breakdown = compute_tier_breakdown(&tier_rows);

    let steam_last_played = read_last_played(steam_root, steamid3, app_id).unwrap_or(0);
    let cached_at = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let name = scanned
        .app_name
        .clone()
        .or_else(|| entry_name.map(|s| s.to_owned()))
        .unwrap_or_else(|| format!("App {app_id}"));

    GameCacheEntry {
        schema_version: cache::CURRENT_SCHEMA_VERSION,
        app_id,
        name,
        steam_last_played,
        cached_at,
        achievements: Vec::new(),
        stats,
        progress: CachedProgress { earned, total },
        tier_breakdown,
        genre: scanned.genre.clone(),
    }
}

fn build_game_view_cache_entry(
    state: &GameViewState,
    app_id: u32,
    steam_root: &std::path::Path,
    steamid3: u64,
) -> GameCacheEntry {
    use cache::types::{CachedAchievement, CachedProgress, CachedStat};
    use steamlens_core::read_last_played;

    let earned = state
        .achievements
        .iter()
        .filter(|r| r.data.is_achieved)
        .count() as u32;
    let total = state.achievements.len() as u32;

    let achievements = state
        .achievements
        .iter()
        .map(|r| CachedAchievement {
            api_name: r.data.id.clone(),
            display_name: r.data.display_name.clone(),
            description: r.data.description.clone(),
            hidden: r.data.is_hidden,
            icon_path: None,
            icon_locked_path: None,
            earned: r.data.is_achieved,
            earned_at: r.data.unlock_time.map(|t| t as u64),
            global_percent: r.rarity_percent.map(|p| p as f64),
        })
        .collect();

    let stats = state
        .stats
        .iter()
        .map(|r| {
            use steamlens_core::StatValue;
            let value = match r.data.value {
                StatValue::Int(i) => cache::types::CachedStatValue::Int(i as i64),
                StatValue::Float(f) => cache::types::CachedStatValue::Float(f as f64),
            };
            CachedStat {
                api_name: r.data.id.clone(),
                display_name: r.data.display_name.clone(),
                value,
            }
        })
        .collect();

    let steam_last_played = read_last_played(steam_root, steamid3, app_id).unwrap_or(0);

    let cached_at = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    GameCacheEntry {
        schema_version: cache::CURRENT_SCHEMA_VERSION,
        app_id,
        name: state.game_name.clone(),
        steam_last_played,
        cached_at,
        achievements,
        stats,
        progress: CachedProgress { earned, total },
        tier_breakdown: state.tier_breakdown.clone(),
        genre: None,
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let skeleton_phase = app.context.animation.skeleton_phase;

    let header: Option<Element<'_, Message>> = match &app.screen {
        Screen::ProfileView(pv_state) => Some(crate::screen::render_app_header(
            profile_view::header_content(pv_state, app.context.connectivity.steam_running),
        )),
        Screen::GameView(state) => Some(crate::screen::render_app_header(
            game_view::header_content(state),
        )),
    };

    let body: Element<'_, Message> = match &app.screen {
        Screen::ProfileView(pv_state) => {
            let props = profile_view::ProfileViewProps {
                user_profile: app.context.user_profile.as_ref(),
                avatar_handle: app.context.profile_avatar_handle.as_ref(),
                cached_entries: &app.context.cached_entries,
                skeleton_phase,
                pinned: &app.context.settings.library.pinned,
                steam_level: app.context.steam_level,
                steam_running: app.context.connectivity.steam_running,
            };
            crate::screen::compose_screen(profile_view::render(pv_state, props))
                .map(Message::ProfileView)
        }

        Screen::GameView(state) => {
            let props = game_view::GameViewProps { skeleton_phase };
            game_view::view(state, props).map(Message::GameView)
        }
    };

    let banner_slot = messaging::banner_stack(&app.context.messaging);

    let mut shell = column![].spacing(0);
    if let Some(h) = header {
        shell = shell.push(h);
    }
    if let Some(b) = banner_slot {
        shell = shell.push(b);
    }
    shell = shell.push(body);
    let shell: Element<'_, Message> = shell.into();

    let with_toasts = messaging::wrap_with_toasts(shell, &app.context.messaging);

    if app.splash_min_elapsed
        && app.library_cache_resolved
        && app.cache_classified
        && app.probe_done
    {
        with_toasts
    } else {
        splash_view()
    }
}

fn splash_view<'a>() -> Element<'a, Message> {
    let title = text("SteamLens")
        .size(40)
        .color(Color::from_rgb(0.741, 0.576, 0.976));
    let subtitle = text("starting up…")
        .size(13)
        .color(Color::from_rgba(0.7, 0.7, 0.78, 0.85));

    let content = column![title, subtitle]
        .spacing(8)
        .align_x(iced::Alignment::Center);

    container(content)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .center_x(iced::Length::Fill)
        .center_y(iced::Length::Fill)
        .style(|_theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.10, 0.08, 0.16))),
            ..Default::default()
        })
        .into()
}

fn has_active_skeletons(app: &App) -> bool {
    match &app.screen {
        Screen::ProfileView(pv) => pv.games.iter().any(|g| !g.is_hydrated()),
        Screen::GameView(state) => {
            matches!(
                state.phase,
                game_view::GameViewPhase::Connecting
                    | game_view::GameViewPhase::WaitingStats
                    | game_view::GameViewPhase::LoadingData
            ) || state
                .achievements
                .iter()
                .any(|r| !r.is_spoiler_hidden() && r.data.icon.is_none())
        }
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    let keyboard_sub = iced::event::listen_with(|event, _status, _id| {
        if let iced::Event::Keyboard(k) = event {
            Some(Message::KeyboardEvent(k))
        } else {
            None
        }
    });

    let poll_sub = if app.context.worker.is_some() {
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::PollWorker)
    } else {
        Subscription::none()
    };

    let skeleton_sub = if has_active_skeletons(app) {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::SkeletonTick)
    } else {
        Subscription::none()
    };

    let settings_flush_sub = if app.context.settings_dirty_since.is_some() {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::SettingsFlushTick)
    } else {
        Subscription::none()
    };

    let toast_sub = if app.context.messaging.has_active_toasts() {
        iced::time::every(Duration::from_millis(500)).map(|_| Message::ToastTick)
    } else {
        Subscription::none()
    };

    let hit_drain_sub = if !app.context.pending_hit_queue.is_empty() {
        iced::time::every(Duration::from_millis(16)).map(|_| Message::DrainHitQueue)
    } else {
        Subscription::none()
    };

    let screen_sub: Subscription<Message> = match &app.screen {
        Screen::ProfileView(state) => {
            profile_view::subscription(state, app.context.connectivity.steam_running)
                .map(Message::ProfileView)
        }
        Screen::GameView(state) => game_view::subscription(state).map(Message::GameView),
    };

    Subscription::batch([
        keyboard_sub,
        poll_sub,
        skeleton_sub,
        settings_flush_sub,
        toast_sub,
        hit_drain_sub,
        screen_sub,
    ])
}

fn theme(_app: &App) -> iced::Theme {
    crate::theme::theme()
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let is_subprocess =
        args.len() >= 2 && (args[1] == "--probe" || args[1].starts_with("--worker"));

    let init_result = if is_subprocess {
        crate::logging::init_worker()
    } else {
        crate::logging::init()
    };
    if let Err(e) = init_result {
        eprintln!("[steamlens] FATAL: logging init failed: {e}");
        std::process::exit(1);
    }

    if args.len() == 2 && args[1] == "--probe" {
        worker::run_probe();
    }
    if args.len() == 3 && args[1] == "--worker" {
        let app_id: u32 = args[2].parse().unwrap_or_else(|_| {
            crate::log!("invalid app_id: {}", args[2]);
            std::process::exit(2);
        });
        worker::run(app_id);
    }
    if args.len() >= 2 && args[1].starts_with("--worker") {
        crate::log!("usage: steamlens-app --worker <app_id>");
        std::process::exit(2);
    }

    let swept = steamlens_core::sweep_orphans();
    if swept > 0 {
        crate::log!("swept {swept} orphan shm region(s) at startup");
    }

    let loaded = settings::load_settings();
    let window_w = loaded.ui.window_width;
    let window_h = loaded.ui.window_height;

    const WINDOW_ICON_BYTES: &[u8] = include_bytes!("../../../assets/icon-256.png");
    let window_icon = iced::window::icon::from_file_data(WINDOW_ICON_BYTES, None).ok();

    iced::application(move || boot_with_settings(loaded.clone()), update, view)
        .title("SteamLens")
        .theme(theme)
        .subscription(subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(window_w.max(896.0), window_h.max(504.0)),
            min_size: Some(iced::Size::new(896.0, 504.0)),
            icon: window_icon,
            ..iced::window::Settings::default()
        })
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for App {
        fn default() -> Self {
            Self {
                context: AppContext {
                    worker: None,
                    worker_rx: None,
                    settings: Settings::default(),
                    settings_dirty_since: None,
                    messaging: MessagingCenter::new(),
                    cached_entries: HashMap::new(),
                    pending_hit_queue: VecDeque::new(),
                    steam_root: std::path::PathBuf::from("/tmp"),
                    steamid3: 0,
                    user_profile: None,
                    profile_avatar_handle: None,
                    connectivity: ConnectivityState {
                        steam_running: Some(true),
                        user_logged_in: Some(true),
                    },
                    steam_level: None,
                    no_ach_cache: cache::NoAchievementsCache::new(),
                    animation: AnimationState {
                        skeleton_phase: 0.0,
                    },
                },
                screen: Screen::ProfileView(Box::new(ProfileViewState::new())),
                splash_min_elapsed: true,
                library_cache_resolved: true,
                cache_classified: true,
                probe_done: true,
            }
        }
    }

    #[tokio::test]
    async fn boot_starts_in_profile_view() {
        let (app, _task) = boot_with_settings(Settings::default());
        assert!(matches!(app.screen, Screen::ProfileView(_)));
        assert!(
            app.context.worker.is_some(),
            "worker must be spawned immediately"
        );
    }

    fn make_app_probing() -> App {
        App {
            screen: Screen::ProfileView(Box::new(ProfileViewState::new())),
            splash_min_elapsed: false,
            library_cache_resolved: false,
            cache_classified: false,
            probe_done: false,
            context: AppContext {
                connectivity: ConnectivityState::default(),
                user_profile: None,
                profile_avatar_handle: None,
                ..App::default().context
            },
        }
    }

    #[test]
    fn probe_result_ok_overrides_profile_and_marks_steam_running() {
        let mut app = make_app_probing();
        let probed = ProbedProfile {
            steam_id: 76561198000000042,
            persona_name: "TestUser".to_owned(),
            avatar_image: Some(vec![0x89, 0x50, 0x4E, 0x47]),
            game_summaries: vec![],
            steam_level: Some(17),
        };
        let _t = update(&mut app, Message::ProbeResult(Ok(probed)));

        assert_eq!(app.context.connectivity.steam_running, Some(true));
        assert_eq!(app.context.connectivity.user_logged_in, Some(true));
        assert!(app.probe_done);
        let profile = app
            .context
            .user_profile
            .as_ref()
            .expect("profile must be set");
        assert_eq!(profile.persona_name, "TestUser");
        assert_eq!(profile.steam_id, 76561198000000042);
        assert_eq!(
            app.context.steamid3,
            76561198000000042 - STEAMID64_INDIVIDUAL_MIN
        );
        assert!(app.context.profile_avatar_handle.is_some());
    }

    #[test]
    fn probe_result_err_preserves_existing_profile() {
        let mut app = make_app_probing();
        let prior = UserProfile {
            steam_id: 1,
            persona_name: "DiskFallback".to_owned(),
            account_name: "fallback".to_owned(),
            avatar_png_bytes: None,
        };
        app.context.user_profile = Some(prior.clone());

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );

        assert_eq!(app.context.connectivity.steam_running, Some(false));
        assert!(app.probe_done);
        let profile = app
            .context
            .user_profile
            .as_ref()
            .expect("profile must be preserved");
        assert_eq!(profile.persona_name, "DiskFallback");
        assert_eq!(profile.steam_id, 1);
    }

    #[test]
    fn probe_result_err_with_no_prior_profile_keeps_none() {
        let mut app = make_app_probing();
        assert!(
            app.context.user_profile.is_none(),
            "precondition: no prior profile"
        );

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::Other("timeout".to_owned()))),
        );

        assert_eq!(app.context.connectivity.steam_running, None);
        assert!(app.probe_done);
        assert!(
            app.context.user_profile.is_none(),
            "no profile should remain None on probe error without disk fallback"
        );
    }

    fn splash_visible(app: &App) -> bool {
        !(app.splash_min_elapsed
            && app.library_cache_resolved
            && app.cache_classified
            && app.probe_done)
    }

    #[test]
    fn splash_stays_until_all_four_signals_arrive() {
        let mut app = make_app_probing();
        assert!(splash_visible(&app), "all four pending → splash visible");

        app.splash_min_elapsed = true;
        assert!(splash_visible(&app), "only min-elapsed → splash visible");

        app.library_cache_resolved = true;
        assert!(
            splash_visible(&app),
            "min+library but no classify+probe → splash visible"
        );

        app.cache_classified = true;
        assert!(
            splash_visible(&app),
            "min+library+classify but no probe → splash visible"
        );

        app.probe_done = true;
        assert!(!splash_visible(&app), "all four done → splash hidden");
    }

    #[test]
    fn splash_hidden_only_after_library_cache_classify_and_probe_resolve() {
        let mut app = make_app_probing();
        app.splash_min_elapsed = true;
        app.library_cache_resolved = true;
        app.cache_classified = true;
        assert!(splash_visible(&app), "missing probe → still visible");

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );
        assert!(
            app.probe_done,
            "probe_done must be set immediately on ProbeResult"
        );
        assert!(
            !splash_visible(&app),
            "splash must dismiss when all four signals are present"
        );
    }

    #[test]
    fn splash_does_not_dismiss_on_probe_failure_until_library_cache_loaded() {
        let mut app = make_app_probing();
        app.splash_min_elapsed = true;

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );

        assert!(app.probe_done, "probe_done must be set on ProbeResult(Err)");
        assert!(
            !app.library_cache_resolved,
            "library_cache_resolved must NOT be set synchronously on probe failure"
        );
        assert!(
            splash_visible(&app),
            "splash must remain visible until library cache lands"
        );

        let cached = CachedLibrary {
            schema_version: 3,
            games: vec![CachedLibraryEntry {
                app_id: 105600,
                change_number: 0,
                last_played: None,
                name: "Terraria".to_owned(),
                achievement_count: 88,
            }],
            cached_at: 0,
        };
        let _t2 = update(&mut app, Message::LibraryCacheLoaded(Some(cached)));

        assert!(
            app.library_cache_resolved,
            "library_cache_resolved must be set after LibraryCacheLoaded"
        );
        assert!(
            splash_visible(&app),
            "splash must remain visible until cache_classified resolves"
        );

        let _t3 = update(
            &mut app,
            Message::CacheClassified(crate::cache::ClassifyResult::default()),
        );
        assert!(
            app.cache_classified,
            "cache_classified must be set after CacheClassified"
        );
        assert!(
            !splash_visible(&app),
            "splash must dismiss after all four signals"
        );
    }

    #[test]
    fn profile_cache_loaded_populates_when_user_profile_is_none() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(false);
        let cached = CachedProfile {
            schema_version: 2,
            steam_id: 76561198000000042,
            persona_name: "FromCache".to_owned(),
            account_name: "cache_login".to_owned(),
            avatar_png_bytes: None,
            cached_at: 0,
            steam_root: None,
        };
        let _t = update(&mut app, Message::ProfileCacheLoaded(Some(cached)));
        let p = app
            .context
            .user_profile
            .as_ref()
            .expect("profile must be set");
        assert_eq!(p.persona_name, "FromCache");
        assert_eq!(p.account_name, "cache_login");
        assert_eq!(
            app.context.steamid3,
            76561198000000042 - STEAMID64_INDIVIDUAL_MIN
        );
    }

    #[test]
    fn profile_cache_loaded_skipped_when_probe_succeeded_first() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(true);
        app.context.user_profile = Some(UserProfile {
            steam_id: 1,
            persona_name: "LiveFromProbe".to_owned(),
            account_name: "live".to_owned(),
            avatar_png_bytes: None,
        });
        let cached = CachedProfile {
            schema_version: 2,
            steam_id: 999,
            persona_name: "ShouldNotWin".to_owned(),
            account_name: "stale".to_owned(),
            avatar_png_bytes: None,
            steam_root: None,
            cached_at: 0,
        };
        let _t = update(&mut app, Message::ProfileCacheLoaded(Some(cached)));
        let p = app.context.user_profile.as_ref().unwrap();
        assert_eq!(
            p.persona_name, "LiveFromProbe",
            "probe-Ok profile must not be overwritten by cache"
        );
    }

    #[test]
    fn profile_cache_loaded_none_is_noop() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(false);
        let _t = update(&mut app, Message::ProfileCacheLoaded(None));
        assert!(app.context.user_profile.is_none());
        assert_eq!(app.context.connectivity.steam_running, Some(false));
    }

    #[test]
    fn library_cache_loaded_some_dispatches_scan_complete_when_games_empty() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(false);
        let cached = CachedLibrary {
            schema_version: 3,
            games: vec![CachedLibraryEntry {
                app_id: 105600,
                change_number: 0,
                last_played: None,
                name: "Terraria".to_owned(),
                achievement_count: 88,
            }],
            cached_at: 0,
        };
        let _t = update(&mut app, Message::LibraryCacheLoaded(Some(cached)));
        if let Screen::ProfileView(pv) = &app.screen {
            assert!(pv.games.is_empty(), "precondition: games empty");
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[test]
    fn library_cache_loaded_skipped_when_games_already_present() {
        use crate::profile_view::types::{CapsuleAsset, GameEntry};
        let mut app = make_app_probing();
        if let Screen::ProfileView(pv) = &mut app.screen {
            pv.games.push(GameEntry {
                app_id: 1,
                change_number: 0,
                last_played: None,
                name: Some("AlreadyHere".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: None,
            });
        }
        let cached = CachedLibrary {
            schema_version: 3,
            games: vec![CachedLibraryEntry {
                app_id: 999,
                change_number: 0,
                last_played: None,
                name: "FromCache".to_owned(),
                achievement_count: 1,
            }],
            cached_at: 0,
        };
        let _t = update(&mut app, Message::LibraryCacheLoaded(Some(cached)));
        if let Screen::ProfileView(pv) = &app.screen {
            assert_eq!(
                pv.games.len(),
                1,
                "no replacement when games already populated"
            );
            assert_eq!(pv.games[0].name.as_deref(), Some("AlreadyHere"));
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[test]
    fn library_cache_loaded_none_is_noop() {
        let mut app = make_app_probing();
        let _t = update(&mut app, Message::LibraryCacheLoaded(None));
        if let Screen::ProfileView(pv) = &app.screen {
            assert!(pv.games.is_empty());
        }
    }

    #[test]
    fn persistent_cache_written_logs_error_but_returns_no_task() {
        let mut app = make_app_probing();
        let _t = update(
            &mut app,
            Message::PersistentCacheWritten("profile", Err("disk full".to_owned())),
        );
        assert_eq!(app.context.connectivity.steam_running, None);
    }

    fn make_game_entry_for_scan(app_id: u32, name: &str) -> crate::profile_view::types::GameEntry {
        use crate::profile_view::types::{CapsuleAsset, GameEntry};
        GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some(name.to_owned()),
            capsule: CapsuleAsset::Pending,
            progress: None,
            genre: None,
        }
    }

    fn make_scanned_data(
        app_name: Option<&str>,
        achievements: Vec<(String, bool, Option<f32>)>,
    ) -> progress_scan::ScannedGameData {
        use std::collections::HashMap;
        use steamlens_core::CardOnlyAchievement;

        let mut percentages = HashMap::new();
        for (id, _, pct) in &achievements {
            if let Some(p) = pct {
                percentages.insert(id.clone(), *p);
            }
        }

        let achievement_data: Vec<CardOnlyAchievement> = achievements
            .into_iter()
            .map(|(id, achieved, _)| CardOnlyAchievement {
                id,
                is_achieved: achieved,
            })
            .collect();

        progress_scan::ScannedGameData {
            app_name: app_name.map(|s| s.to_owned()),
            achievements: achievement_data,
            stats: Vec::new(),
            global_percentages: percentages,
            genre: None,
        }
    }

    #[test]
    fn build_cache_entry_from_scan_counts_progress_correctly() {
        let scanned = make_scanned_data(
            Some("Terraria"),
            vec![
                ("ACH_A".to_owned(), true, Some(50.0)),
                ("ACH_B".to_owned(), false, Some(10.0)),
                ("ACH_C".to_owned(), true, Some(5.0)),
            ],
        );
        let game = make_game_entry_for_scan(105600, "TerrariaFallback");
        let entry = build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            std::path::Path::new("/tmp/nonexistent"),
            0,
        );
        assert_eq!(entry.progress.earned, 2);
        assert_eq!(entry.progress.total, 3);
        assert_eq!(entry.app_id, 105600);
        assert_eq!(
            entry.name, "Terraria",
            "scanner-supplied name takes priority over entry name"
        );
    }

    #[test]
    fn build_cache_entry_from_scan_falls_back_to_entry_name() {
        let scanned = make_scanned_data(None, vec![("X".to_owned(), false, None)]);
        let game = make_game_entry_for_scan(1, "FallbackName");
        let entry = build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            std::path::Path::new("/tmp/nonexistent"),
            0,
        );
        assert_eq!(entry.name, "FallbackName");
    }

    #[test]
    fn build_cache_entry_from_scan_computes_tier_breakdown_when_pct_present() {
        let scanned = make_scanned_data(
            None,
            vec![
                ("A".to_owned(), true, Some(1.0)),
                ("B".to_owned(), true, Some(50.0)),
                ("C".to_owned(), false, Some(99.0)),
                ("D".to_owned(), true, Some(20.0)),
            ],
        );
        let game = make_game_entry_for_scan(99, "Game");
        let entry = build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            std::path::Path::new("/tmp/nonexistent"),
            0,
        );
        assert!(
            !entry.tier_breakdown.is_empty(),
            "tier_breakdown must be populated when global_percentages present"
        );
    }

    #[test]
    fn build_cache_entry_from_scan_empty_tier_breakdown_without_pct() {
        let scanned = make_scanned_data(
            None,
            vec![("A".to_owned(), true, None), ("B".to_owned(), false, None)],
        );
        let game = make_game_entry_for_scan(99, "Game");
        let entry = build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            std::path::Path::new("/tmp/nonexistent"),
            0,
        );
        assert!(
            entry.tier_breakdown.is_empty(),
            "no global_percent → no tier classification"
        );
    }

    #[test]
    fn drain_progress_results_failure_records_failed_app_id() {
        use crate::profile_view::types::{CapsuleAsset, GameEntry};
        use crate::progress_scan::ProgressResult;

        let mut app = make_app_probing();
        if let Screen::ProfileView(pv) = &mut app.screen {
            pv.games.push(GameEntry {
                app_id: 105600,
                change_number: 0,
                last_played: None,
                name: Some("Terraria".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: None,
            });
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            tx.send(ProgressResult {
                app_id: 105600,
                data: None,
            })
            .unwrap();
            drop(tx);
            pv.progress_rx = Some(rx);
        }

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::DrainProgressResults),
        );

        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.failed_app_ids.contains(&105600),
                "failed app_id must be recorded after None scan result"
            );
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[test]
    fn loader_phase_failed_when_some_games_have_no_progress_and_failed() {
        use crate::profile_view::types::{CapsuleAsset, GameEntry, LoaderPhase, ProfileViewState};
        use crate::progress_scan::ProgressData;

        let mk_entry = |app_id: u32, with_progress: bool| GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some(format!("Game {app_id}")),
            capsule: CapsuleAsset::Unavailable,
            progress: if with_progress {
                Some(ProgressData {
                    earned: 1,
                    total: 1,
                })
            } else {
                None
            },
            genre: None,
        };

        let mut state = ProfileViewState::new();
        state.games.push(mk_entry(1, true));
        state.games.push(mk_entry(2, true));
        state.games.push(mk_entry(3, false));
        state.failed_app_ids.insert(3);

        assert_eq!(
            state.loader_phase(Some(true)),
            LoaderPhase::Failed {
                failed: 1,
                total: 3
            },
            "all games accounted for (2 progress + 1 failed) → Failed phase"
        );
    }

    #[test]
    fn loader_phase_steam_off_when_no_games_and_steam_off() {
        use crate::profile_view::types::{LoaderPhase, ProfileViewState};
        let state = ProfileViewState::new();
        assert_eq!(state.loader_phase(Some(false)), LoaderPhase::SteamOff);
    }

    #[test]
    fn loader_phase_alpha_when_no_games_and_steam_unknown() {
        use crate::profile_view::types::{LoaderPhase, ProfileViewState};
        let state = ProfileViewState::new();
        assert_eq!(
            state.loader_phase(None),
            LoaderPhase::Alpha,
            "steam_running=None during boot probe → Alpha not SteamOff"
        );
    }

    #[test]
    fn retry_failed_scans_clears_set_and_spawns_scanner() {
        let mut app = make_app_probing();
        if let Screen::ProfileView(pv) = &mut app.screen {
            pv.failed_app_ids.insert(105600);
            pv.failed_app_ids.insert(570);
        }

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::RetryFailedScans),
        );

        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.failed_app_ids.is_empty(),
                "failed set must be cleared after retry"
            );
            assert!(pv.progress_scanner.is_some(), "new scanner must be spawned");
            assert!(
                pv.progress_rx.is_some(),
                "progress_rx must be wired to new scanner"
            );
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[test]
    fn retry_failed_scans_noop_when_no_failures() {
        let mut app = make_app_probing();
        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::RetryFailedScans),
        );
        if let Screen::ProfileView(pv) = &app.screen {
            assert!(pv.progress_scanner.is_none(), "no scanner spawned");
        }
    }

    #[test]
    fn retry_steam_connect_resets_connectivity() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(false);
        app.context.connectivity.user_logged_in = Some(false);

        let _t = update(&mut app, Message::RetrySteamConnect);

        assert_eq!(
            app.context.connectivity.steam_running, None,
            "connectivity.steam_running reset to None during re-probe"
        );
        assert_eq!(
            app.context.connectivity.user_logged_in, None,
            "connectivity.user_logged_in reset to None during re-probe"
        );
    }

    #[test]
    fn account_name_preserved_when_probe_succeeds() {
        let mut app = make_app_probing();
        app.context.user_profile = Some(UserProfile {
            steam_id: 1,
            persona_name: "OldName".to_owned(),
            account_name: "preserved_login".to_owned(),
            avatar_png_bytes: None,
        });

        let probed = ProbedProfile {
            steam_id: 76561198000000042,
            persona_name: "LiveName".to_owned(),
            avatar_image: None,
            game_summaries: vec![],
            steam_level: None,
        };
        let _t = update(&mut app, Message::ProbeResult(Ok(probed)));

        let p = app.context.user_profile.as_ref().unwrap();
        assert_eq!(p.persona_name, "LiveName", "persona overridden by probe");
        assert_eq!(
            p.account_name, "preserved_login",
            "account_name from disk preserved (probe doesn't fetch it)"
        );
    }

    #[test]
    fn game_view_state_dirty_count_zero_on_init() {
        let state = GameViewState::new(105600);
        assert_eq!(state.dirty_count(), 0);
    }

    #[test]
    fn game_view_state_no_errors_on_init() {
        let state = GameViewState::new(105600);
        assert!(!state.has_stat_errors());
    }

    fn make_game_view_state_with_dirty_achievements() -> GameViewState {
        use game_view::types::{AchievementData, AchievementRow};
        let mut state = GameViewState::new(105600);
        state.phase = game_view::GameViewPhase::Ready;
        let data = AchievementData {
            id: "ACH_1".to_owned(),
            display_name: "Test".to_owned(),
            description: "desc".to_owned(),
            is_hidden: false,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        };
        let mut row = AchievementRow::from(data);
        row.is_dirty = true;
        state.achievements.push(row);
        state
    }

    #[test]
    fn cancel_resets_dirty_count_to_zero() {
        use game_view::GameViewMessage;

        let mut state = make_game_view_state_with_dirty_achievements();
        assert_eq!(
            state.dirty_count(),
            1,
            "precondition: one dirty achievement"
        );

        let mut app = App::default();
        let _task = game_view::update(
            &mut state,
            GameViewMessage::DiscardChanges,
            &mut app.context,
        );

        assert_eq!(
            state.dirty_count(),
            0,
            "dirty count must be zero after DiscardChanges"
        );
    }

    #[test]
    fn cancel_does_not_change_phase() {
        use game_view::GameViewMessage;

        let mut state = make_game_view_state_with_dirty_achievements();
        assert_eq!(state.phase, game_view::GameViewPhase::Ready, "precondition");

        let mut app = App::default();
        let _task = game_view::update(
            &mut state,
            GameViewMessage::DiscardChanges,
            &mut app.context,
        );

        assert_eq!(
            state.phase,
            game_view::GameViewPhase::Ready,
            "phase must not change after DiscardChanges"
        );
    }

    #[test]
    fn game_view_state_app_name_starts_as_fallback() {
        let state = GameViewState::new(105600);
        assert_eq!(
            state.game_name, "App 105600",
            "initial game_name must be fallback App <id>"
        );
    }

    #[test]
    fn connected_reply_updates_game_name() {
        use game_view::handle_steam_reply;
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(105600);
        let _task = handle_steam_reply(
            &mut state,
            SteamReply::Connected {
                app_name: Some("Terraria".to_owned()),
            },
        );
        assert_eq!(
            state.game_name, "Terraria",
            "game_name must update when Connected reply carries app_name"
        );
    }

    #[test]
    fn connected_reply_keeps_fallback_when_app_name_none() {
        use game_view::handle_steam_reply;
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(105600);
        let _task = handle_steam_reply(&mut state, SteamReply::Connected { app_name: None });
        assert_eq!(
            state.game_name, "App 105600",
            "game_name must remain fallback when app_name is None"
        );
    }

    #[test]
    fn reveal_hidden_sets_revealed_true() {
        use game_view::types::{AchievementData, AchievementRow};
        use game_view::{GameViewMessage, GameViewPhase, update};

        let mut state = GameViewState::new(105600);
        state.phase = GameViewPhase::Ready;
        let data = AchievementData {
            id: "ACH_SECRET".to_owned(),
            display_name: "Secret".to_owned(),
            description: "desc".to_owned(),
            is_hidden: true,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        };
        state.achievements.push(AchievementRow::from(data));
        assert!(
            !state.achievements[0].revealed,
            "precondition: revealed must be false"
        );

        let mut app = App::default();
        let _task = update(
            &mut state,
            GameViewMessage::RevealHidden("ACH_SECRET".to_owned()),
            &mut app.context,
        );

        assert!(
            state.achievements[0].revealed,
            "revealed must be true after RevealHidden"
        );
    }

    #[test]
    fn sort_orders_unlocked_then_locked_then_hidden() {
        use game_view::types::{
            AchievementData, AchievementFilter, AchievementRow, AchievementSort,
            visible_achievement_ids,
        };

        fn row(
            id: &str,
            name: &str,
            is_achieved: bool,
            is_hidden: bool,
            revealed: bool,
        ) -> AchievementRow {
            let mut r = AchievementRow::from(AchievementData {
                id: id.to_owned(),
                display_name: name.to_owned(),
                description: String::new(),
                is_hidden,
                is_achieved,
                unlock_time: None,
                permission: 0,
                icon: None,
            });
            r.revealed = revealed;
            r.appeared = true;
            r
        }

        let achievements = vec![
            row("A", "Alpha", true, false, false),
            row("B", "Beta", false, false, false),
            row("C", "Gamma", false, true, false),
            row("D", "Delta", false, true, true),
            row("E", "Epsilon", true, true, false),
        ];

        let ids = visible_achievement_ids(
            &achievements,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &std::collections::HashSet::new(),
            false,
        );

        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], "A", "A (unlocked) first");
        assert_eq!(ids[1], "E", "E (hidden+achieved = unlocked) second");
        assert_eq!(ids[2], "B", "B (locked) before D (revealed)");
        assert_eq!(ids[3], "D", "D (revealed hidden = locked group)");
        assert_eq!(ids[4], "C", "C (hidden unrevealed) last");
    }

    #[test]
    fn dirty_unlock_does_not_change_group_until_apply() {
        use game_view::types::{
            AchievementData, AchievementFilter, AchievementRow, AchievementSort,
            visible_achievement_ids,
        };

        let mut zebra = AchievementRow::from(AchievementData {
            id: "ZEBRA".to_owned(),
            display_name: "Zebra".to_owned(),
            description: String::new(),
            is_hidden: false,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        });
        zebra.is_dirty = true;
        zebra.appeared = true;

        let mut ant = AchievementRow::from(AchievementData {
            id: "ANT".to_owned(),
            display_name: "Ant".to_owned(),
            description: String::new(),
            is_hidden: false,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        });
        ant.appeared = true;

        let achievements = vec![zebra, ant];
        let ids = visible_achievement_ids(
            &achievements,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &std::collections::HashSet::new(),
            false,
        );

        assert_eq!(
            ids[0], "ANT",
            "Ant comes first alphabetically — both rows are in the Locked group until Apply persists the change"
        );
        assert_eq!(
            ids[1], "ZEBRA",
            "Zebra stays in Locked group despite dirty=true; sort uses persisted is_achieved only"
        );
    }

    #[test]
    fn case_insensitive_sort() {
        use game_view::types::{
            AchievementData, AchievementFilter, AchievementRow, AchievementSort,
            visible_achievement_ids,
        };

        fn unlocked_row(id: &str, name: &str) -> AchievementRow {
            let mut r = AchievementRow::from(AchievementData {
                id: id.to_owned(),
                display_name: name.to_owned(),
                description: String::new(),
                is_hidden: false,
                is_achieved: true,
                unlock_time: None,
                permission: 0,
                icon: None,
            });
            r.appeared = true;
            r
        }

        let achievements = vec![
            unlocked_row("C", "cherry"),
            unlocked_row("B", "Banana"),
            unlocked_row("A", "apple"),
        ];

        let ids = visible_achievement_ids(
            &achievements,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &std::collections::HashSet::new(),
            false,
        );

        assert_eq!(ids[0], "A", "apple first (case-insensitive)");
        assert_eq!(ids[1], "B", "Banana second");
        assert_eq!(ids[2], "C", "cherry third");
    }

    #[test]
    fn reset_confirm_blocks_when_name_mismatch() {
        use game_view::{GameViewMessage, GameViewPhase, update};

        let mut state = GameViewState::new(105600);
        state.phase = GameViewPhase::Ready;
        state.game_name = "Terraria".to_owned();

        let mut app = App::default();
        let _task = update(&mut state, GameViewMessage::ResetClicked, &mut app.context);
        assert!(state.show_reset_modal, "modal must open on ResetClicked");
        assert!(
            state.reset_confirm_input.is_empty(),
            "input must be cleared on open"
        );

        let _task = update(
            &mut state,
            GameViewMessage::ResetConfirmInputChanged("Wrong".to_owned()),
            &mut app.context,
        );
        assert_eq!(state.reset_confirm_input, "Wrong");
        assert!(
            !state.reset_confirm_matches(),
            "confirm must NOT match for wrong input"
        );
    }

    #[test]
    fn reset_confirm_allows_case_insensitive_match() {
        use game_view::{GameViewMessage, GameViewPhase, update};

        let mut state = GameViewState::new(105600);
        state.phase = GameViewPhase::Ready;
        state.game_name = "Terraria".to_owned();

        let mut app = App::default();
        let _task = update(&mut state, GameViewMessage::ResetClicked, &mut app.context);

        let _task = update(
            &mut state,
            GameViewMessage::ResetConfirmInputChanged("TERRARIA ".to_owned()),
            &mut app.context,
        );
        assert!(
            state.reset_confirm_matches(),
            "confirm must match for case-insensitive + trailing space input"
        );
    }

    #[test]
    fn global_percentages_reply_populates_rarity() {
        use std::collections::HashMap;

        use game_view::handle_steam_reply;
        use game_view::types::{AchievementData, AchievementRow};
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(0);

        let make_row = |id: &str| {
            AchievementRow::from(AchievementData {
                id: id.to_owned(),
                display_name: id.to_owned(),
                description: String::new(),
                is_hidden: false,
                is_achieved: false,
                unlock_time: None,
                permission: 0,
                icon: None,
            })
        };

        state.achievements = vec![make_row("ACH_RARE"), make_row("ACH_COMMON")];

        let mut map = HashMap::new();
        map.insert("ACH_RARE".to_owned(), 4.0f32);
        map.insert("ACH_COMMON".to_owned(), 55.0f32);

        let _task = handle_steam_reply(&mut state, SteamReply::GlobalPercentagesReady(map));

        assert_eq!(
            state.achievements[0].rarity_percent,
            Some(4.0),
            "ACH_RARE must have rarity_percent = Some(4.0)"
        );
        assert_eq!(
            state.achievements[1].rarity_percent,
            Some(55.0),
            "ACH_COMMON must have rarity_percent = Some(55.0)"
        );
    }

    #[test]
    fn apply_then_reload_preserves_revealed_state() {
        use game_view::handle_steam_reply;
        use game_view::types::{AchievementData, AchievementRow};
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(105600);
        let data = AchievementData {
            id: "ACH_HIDDEN".to_owned(),
            display_name: "Spoiler".to_owned(),
            description: "hidden".to_owned(),
            is_hidden: true,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        };
        let mut row = AchievementRow::from(data.clone());
        row.revealed = true;
        state.achievements.push(row);

        let _task = handle_steam_reply(
            &mut state,
            SteamReply::AchievementsAndStats {
                achievements: vec![data],
                stats: vec![],
            },
        );

        assert!(
            state.achievements[0].revealed,
            "revealed state must survive AchievementsAndStats refresh"
        );
    }

    fn make_game_entry(app_id: u32, change_number: u32) -> profile_view::types::GameEntry {
        use profile_view::types::{CapsuleAsset, GameEntry};
        GameEntry {
            app_id,
            change_number,
            last_played: None,
            name: None,
            capsule: CapsuleAsset::Unavailable,
            progress: None,
            genre: None,
        }
    }

    #[tokio::test]
    async fn drain_progress_success_path_inserts_cached_entry() {
        use steamlens_core::CardOnlyAchievement;

        let app_id: u32 = 105600;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<progress_scan::ProgressResult>();
        let mut pv_state = ProfileViewState::new();
        pv_state.games.push(make_game_entry(app_id, 7));
        pv_state.progress_rx = Some(rx);

        let mut app = App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            ..App::default()
        };

        tx.send(progress_scan::ProgressResult {
            app_id,
            data: Some(progress_scan::ScannedGameData {
                app_name: Some("Terraria".to_owned()),
                achievements: vec![CardOnlyAchievement {
                    id: "ACH_KILL_BOSS".to_owned(),
                    is_achieved: true,
                }],
                stats: vec![],
                global_percentages: HashMap::new(),
                genre: None,
            }),
        })
        .expect("send result");

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::DrainProgressResults),
        );

        assert!(
            app.context.cached_entries.contains_key(&app_id),
            "successful scan must insert into cached_entries"
        );
        if let Screen::ProfileView(pv) = &app.screen {
            let game = pv
                .games
                .iter()
                .find(|g| g.app_id == app_id)
                .expect("game still in library");
            assert_eq!(
                game.name.as_deref(),
                Some("Terraria"),
                "game name must be hydrated from scanned data"
            );
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[tokio::test]
    async fn drain_progress_empty_achievements_marks_no_ach_cache() {
        let app_id: u32 = 99999;
        let change_number: u32 = 42;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<progress_scan::ProgressResult>();
        let mut pv_state = ProfileViewState::new();
        pv_state.games.push(make_game_entry(app_id, change_number));
        pv_state.progress_rx = Some(rx);

        let mut app = App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            ..App::default()
        };

        tx.send(progress_scan::ProgressResult {
            app_id,
            data: Some(progress_scan::ScannedGameData {
                app_name: None,
                achievements: vec![],
                stats: vec![],
                global_percentages: HashMap::new(),
                genre: None,
            }),
        })
        .expect("send result");

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::DrainProgressResults),
        );

        assert_eq!(
            app.context.no_ach_cache.entries.get(&app_id).copied(),
            Some(change_number),
            "empty-achievements scan must record (app_id, change_number) in no_ach_cache"
        );
        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.games.iter().all(|g| g.app_id != app_id),
                "empty-achievements scan must remove game from library"
            );
        } else {
            panic!("expected ProfileView screen");
        }
        assert!(
            !app.context.cached_entries.contains_key(&app_id),
            "empty-achievements scan must drop any cached_entries for the app"
        );
    }

    #[tokio::test]
    async fn drain_progress_failed_scan_records_failed_app_id() {
        let app_id: u32 = 12345;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<progress_scan::ProgressResult>();
        let mut pv_state = ProfileViewState::new();
        pv_state.games.push(make_game_entry(app_id, 0));
        pv_state.progress_rx = Some(rx);

        let mut app = App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            ..App::default()
        };

        tx.send(progress_scan::ProgressResult { app_id, data: None })
            .expect("send result");

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::DrainProgressResults),
        );

        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.failed_app_ids.contains(&app_id),
                "failed scan must record app_id in failed_app_ids"
            );
        } else {
            panic!("expected ProfileView screen");
        }
    }

    #[tokio::test]
    async fn open_game_view_then_go_back_round_trip() {
        let app_id: u32 = 440;
        let pv_state = ProfileViewState::new();
        let mut app = App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            ..App::default()
        };
        app.context.cached_entries.insert(
            app_id,
            cache::GameCacheEntry {
                schema_version: cache::CURRENT_SCHEMA_VERSION,
                app_id,
                name: "Team Fortress 2".to_owned(),
                steam_last_played: 0,
                cached_at: 1_000,
                achievements: vec![],
                stats: vec![],
                progress: cache::types::CachedProgress {
                    earned: 10,
                    total: 520,
                },
                tier_breakdown: Vec::new(),
                genre: None,
            },
        );

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::RequestOpenGame(app_id)),
        );
        assert!(
            matches!(app.screen, Screen::GameView(_)),
            "RequestOpenGame must switch to GameView screen"
        );
        assert!(
            app.context.worker.is_some(),
            "RequestOpenGame must respawn worker for the new app"
        );
        assert!(
            app.context.cached_entries.contains_key(&app_id),
            "cached entry must survive RequestOpenGame"
        );

        if let Screen::GameView(gv) = &app.screen {
            assert!(
                gv.prev_profile_state.games.is_empty(),
                "prev_profile_state must be stored in GameViewState"
            );
        } else {
            panic!("expected GameView screen");
        }

        let _t = update(&mut app, Message::GameView(GameViewMessage::RequestGoBack));
        assert!(
            matches!(app.screen, Screen::ProfileView(_)),
            "RequestGoBack from GameView must restore ProfileView screen"
        );
        assert!(
            app.context.cached_entries.contains_key(&app_id),
            "cached entry must survive GoBack (overwritten with fresh game-view snapshot)"
        );
    }

    fn make_app_with_game_view_phase(phase: game_view::GameViewPhase) -> App {
        let mut state = GameViewState::new(570);
        state.phase = phase;
        App {
            screen: Screen::GameView(Box::new(state)),
            ..App::default()
        }
    }

    #[test]
    fn has_active_skeletons_true_for_game_view_waiting_stats() {
        let app = make_app_with_game_view_phase(game_view::GameViewPhase::WaitingStats);
        assert!(
            has_active_skeletons(&app),
            "WaitingStats must activate skeleton subscription"
        );
    }

    #[test]
    fn has_active_skeletons_true_for_game_view_connecting() {
        let app = make_app_with_game_view_phase(game_view::GameViewPhase::Connecting);
        assert!(
            has_active_skeletons(&app),
            "Connecting must activate skeleton subscription"
        );
    }

    #[test]
    fn has_active_skeletons_true_for_game_view_loading_data() {
        let app = make_app_with_game_view_phase(game_view::GameViewPhase::LoadingData);
        assert!(
            has_active_skeletons(&app),
            "LoadingData must activate skeleton subscription"
        );
    }

    #[test]
    fn has_active_skeletons_false_for_game_view_ready() {
        let app = make_app_with_game_view_phase(game_view::GameViewPhase::Ready);
        assert!(
            !has_active_skeletons(&app),
            "Ready phase must NOT activate skeleton subscription"
        );
    }

    fn make_app_with_n_games(n: u32) -> App {
        let mut pv_state = ProfileViewState::new();
        for i in 1..=n {
            pv_state.games.push(make_game_entry(i, 0));
        }
        App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            ..App::default()
        }
    }

    fn make_classify_result(hit_ids: &[u32], dirty_ids: &[u32]) -> ClassifyResult {
        use cache::CacheHit;
        use cache::types::{CURRENT_SCHEMA_VERSION, CachedProgress, GameCacheEntry};
        let hits = hit_ids
            .iter()
            .map(|&app_id| CacheHit {
                app_id,
                entry: GameCacheEntry {
                    schema_version: CURRENT_SCHEMA_VERSION,
                    app_id,
                    name: format!("Game {app_id}"),
                    steam_last_played: 0,
                    cached_at: 1_000,
                    achievements: vec![],
                    stats: vec![],
                    progress: CachedProgress {
                        earned: 1,
                        total: 10,
                    },
                    tier_breakdown: vec![],
                    genre: None,
                },
            })
            .collect();
        ClassifyResult {
            hits,
            dirty: dirty_ids.to_vec(),
            schema_bumped: 0,
            invalidation_count: dirty_ids.len() as u32,
        }
    }

    #[test]
    fn cache_classified_all_valid_marks_last_scan_completed() {
        let hit_ids: Vec<u32> = (1..=300).collect();
        let mut app = make_app_with_n_games(300);
        let result = make_classify_result(&hit_ids, &[]);
        let _t = update(&mut app, Message::CacheClassified(result));
        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.last_scan_completed_at.is_some(),
                "all-valid path must mark last_scan_completed_at"
            );
        } else {
            panic!("expected ProfileView");
        }
    }
}
