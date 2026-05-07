mod cache;
mod capsule_cache;
mod game_view;
mod ipc_pipe;
mod messaging;
mod profile_view;
mod progress_scan;
mod settings;
mod skeleton;
mod steam_worker;
mod theme;
mod timeouts;
mod worker;

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use iced::keyboard;
use iced::widget::{button, center, column, container, row, text};
use iced::{Color, Element, Subscription, Task};

use cache::{
    CacheHit, CachedLibrary, CachedLibraryEntry, CachedProfile, ClassifyResult, GameCacheEntry,
};
use game_view::{GameViewMessage, GameViewState};
use messaging::{BannerSeverity, FooterStatus, MessagingCenter, ToastKind};
use profile_view::types::{ProfileViewMessage, ProfileViewState};
use settings::Settings;
use steam_worker::{SteamReply, SteamRequest, SteamWorker};
use steamlens_core::{ProbedProfile, STEAMID64_INDIVIDUAL_MIN, UserProfile};

#[derive(Debug)]
enum Screen {
    ProfileView(Box<ProfileViewState>),
    SteamNotRunning { reason: String },
    GameView(Box<GameViewState>),
}

#[derive(Debug, Clone)]
enum Message {
    Exit,
    GoBack,
    ProfileView(ProfileViewMessage),
    OpenGameView(u32),
    GameView(GameViewMessage),
    PollWorker,
    KeyboardEvent(keyboard::Event),
    DrainProgressResults,
    SplashMinElapsed,
    ProbeResult(Result<ProbedProfile, String>),
    RetrySteamConnect,
    ProfileCacheLoaded(Option<CachedProfile>),
    LibraryCacheLoaded(Option<CachedLibrary>),
    PersistentCacheWritten(&'static str, Result<(), String>),
    SettingsFlushTick,
    SettingsWritten(Result<(), String>),
    ToastRequest(String),
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
    #[allow(dead_code)]
    ClearAllCache,
    #[allow(dead_code)]
    ClearGameCache(u32),
    SkeletonTick,
    FocusSearch,
    ToggleGamePin(u32),
    NoAchCacheLoaded(cache::NoAchievementsCache),
    NoAchCacheWritten(Result<(), String>),
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
    screen: Screen,
    worker: Option<SteamWorker>,
    worker_rx: Option<mpsc::Receiver<SteamReply>>,
    profile_view_state: Option<Box<ProfileViewState>>,
    settings: Settings,
    settings_dirty_since: Option<Instant>,
    messaging: MessagingCenter,
    cached_entries: HashMap<u32, GameCacheEntry>,
    pending_hit_queue: VecDeque<CacheHit>,
    steam_root: std::path::PathBuf,
    steamid3: u64,
    user_profile: Option<UserProfile>,
    profile_avatar_handle: Option<iced::widget::image::Handle>,
    splash_min_elapsed: bool,
    splash_scan_done: bool,
    splash_probe_done: bool,
    steam_running: Option<bool>,
    steam_level: Option<u32>,
    skeleton_phase: f32,
    no_ach_cache: cache::NoAchievementsCache,
    library_name_map: HashMap<u32, String>,
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
    pv_state.search = loaded_settings.library.search.clone();

    let (worker, rx) = SteamWorker::spawn();

    let user_profile = profile_result.ok();
    let profile_avatar_handle = user_profile
        .as_ref()
        .and_then(|p| p.avatar_png_bytes.as_ref())
        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));

    let app = App {
        screen: Screen::ProfileView(Box::new(pv_state)),
        worker: Some(worker),
        worker_rx: Some(rx),
        profile_view_state: None,
        settings: loaded_settings,
        settings_dirty_since: None,
        messaging: MessagingCenter::new(),
        cached_entries: HashMap::new(),
        pending_hit_queue: VecDeque::new(),
        steam_root,
        steamid3,
        user_profile,
        profile_avatar_handle,
        splash_min_elapsed: false,
        splash_scan_done: false,
        splash_probe_done: false,
        steam_running: None,
        steam_level: None,
        skeleton_phase: 0.0,
        no_ach_cache: cache::NoAchievementsCache::new(),
        library_name_map: HashMap::new(),
    };

    let min_splash_task = Task::perform(
        async { tokio::time::sleep(std::time::Duration::from_millis(750)).await },
        |_| Message::SplashMinElapsed,
    );

    let probe_task = Task::perform(
        async {
            steamlens_core::probe_steam(timeouts::PROBE_STEAM_BOOT)
                .await
                .map_err(|e| e.to_string())
        },
        Message::ProbeResult,
    );

    let no_ach_load_task = Task::perform(
        cache::load_no_achievements_cache(),
        Message::NoAchCacheLoaded,
    );

    (
        app,
        Task::batch([min_splash_task, probe_task, no_ach_load_task]),
    )
}

fn drain_worker_replies(app: &mut App) -> Task<Message> {
    let Some(rx) = &app.worker_rx else {
        return Task::none();
    };

    let replies: Vec<SteamReply> = rx.try_iter().collect();
    let mut tasks: Vec<Task<Message>> = Vec::new();

    for reply in replies {
        if let SteamReply::ConnectFailed(reason) = &reply {
            app.screen = Screen::SteamNotRunning {
                reason: reason.clone(),
            };
            disconnect_worker(app);
            return Task::none();
        }

        if let SteamReply::Connected { .. } = &reply
            && let Some(w) = &app.worker
        {
            w.send(SteamRequest::RequestUserStats);
            w.send(SteamRequest::RequestGlobalPercentages);
        }

        if let SteamReply::ResetDone = &reply
            && let Some(w) = &app.worker
        {
            w.send(SteamRequest::RequestUserStats);
        }

        let Screen::GameView(state) = &mut app.screen else {
            continue;
        };

        let t = game_view::handle_steam_reply(state, reply);
        tasks.push(t);
    }

    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

fn mark_settings_dirty(app: &mut App) {
    if app.settings_dirty_since.is_none() {
        app.settings_dirty_since = Some(Instant::now());
    }
}

fn disconnect_worker(app: &mut App) {
    if let Some(w) = &app.worker {
        w.send(SteamRequest::Disconnect);
    }
    app.worker = None;
    app.worker_rx = None;
}

fn return_to_profile_view(app: &mut App) {
    disconnect_worker(app);
    if let Some(stored) = app.profile_view_state.take() {
        app.screen = Screen::ProfileView(stored);
    } else {
        let pv_state = ProfileViewState::new();
        let (worker, rx) = SteamWorker::spawn();
        app.worker = Some(worker);
        app.worker_rx = Some(rx);
        app.screen = Screen::ProfileView(Box::new(pv_state));
    }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Exit => iced::exit(),

        Message::GoBack => {
            match &app.screen {
                Screen::GameView(_) => {
                    let write_task = if let Screen::GameView(state) = &app.screen {
                        let app_id = state.app_id;
                        let entry = build_game_view_cache_entry(
                            state,
                            app_id,
                            &app.steam_root,
                            app.steamid3,
                        );
                        app.cached_entries.insert(app_id, entry.clone());
                        Task::perform(
                            async move {
                                cache::write_game_cache(&entry)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            move |result| Message::CacheWritten { app_id, result },
                        )
                    } else {
                        Task::none()
                    };

                    return_to_profile_view(app);
                    return write_task;
                }
                Screen::SteamNotRunning { .. } => {
                    return_to_profile_view(app);
                }
                _ => {}
            }
            Task::none()
        }

        Message::ProfileView(pv_msg) => {
            match &pv_msg {
                ProfileViewMessage::GameSelected(app_id) => {
                    let app_id = *app_id;
                    if app_id == 0 {
                        return Task::none();
                    }
                    return update(app, Message::OpenGameView(app_id));
                }
                ProfileViewMessage::RescanRequested => {
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        let t = profile_view::update(pv_state, pv_msg);
                        let probe_task = Task::perform(
                            async {
                                steamlens_core::probe_steam(timeouts::PROBE_STEAM_RECONNECT)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            Message::ProbeResult,
                        );
                        return Task::batch([t, probe_task]);
                    }
                    return Task::none();
                }
                ProfileViewMessage::SortChanged(new_sort) => {
                    app.settings.library.sort = *new_sort;
                    mark_settings_dirty(app);
                }
                ProfileViewMessage::SearchChanged(new_search) => {
                    app.settings.library.search = new_search.clone();
                    mark_settings_dirty(app);
                }
                ProfileViewMessage::ProgressFetched { .. } => {
                    let (current, total) = if let Screen::ProfileView(pv) = &app.screen {
                        let with_prog = pv.games.iter().filter(|g| g.progress.is_some()).count();
                        (with_prog, pv.games.len())
                    } else {
                        (0, 0)
                    };
                    app.messaging.footer = FooterStatus::Scanning {
                        current,
                        total,
                        label: "Loading achievements\u{2026}".to_owned(),
                    };
                }
                ProfileViewMessage::ProgressScanDone => {
                    let games = if let Screen::ProfileView(pv) = &app.screen {
                        pv.games.len()
                    } else {
                        0
                    };
                    app.messaging.footer = FooterStatus::Connected {
                        games,
                        last_sync: Some(std::time::Instant::now()),
                    };
                }
                ProfileViewMessage::ScanFailed(reason) => {
                    let is_first = if let Screen::ProfileView(pv_state) = &app.screen {
                        pv_state.failed_app_ids.len() == 1
                    } else {
                        false
                    };
                    if is_first {
                        app.messaging.push_banner(
                            BannerSeverity::Warning,
                            reason.clone(),
                            None,
                            true,
                        );
                    }
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        return profile_view::update(pv_state, pv_msg);
                    }
                    return Task::none();
                }
                ProfileViewMessage::RetryFailedScans => {
                    let failed_ids: Vec<u32> =
                        if let Screen::ProfileView(pv_state) = &mut app.screen {
                            let ids: Vec<u32> = pv_state.failed_app_ids.iter().copied().collect();
                            pv_state.failed_app_ids.clear();
                            ids
                        } else {
                            Vec::new()
                        };
                    if failed_ids.is_empty() {
                        return Task::none();
                    }
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        let mut scanner = crate::progress_scan::ProgressScanner::new(failed_ids);
                        pv_state.progress_rx = scanner.take_receiver();
                        pv_state.progress_scanner = Some(scanner);
                    }
                    return Task::none();
                }
                ProfileViewMessage::ScanComplete(enumerated) => {
                    app.splash_scan_done = true;
                    let games = enumerated.clone();
                    let steam_root = app.steam_root.clone();
                    let steamid3 = app.steamid3;
                    let classify_task = Task::perform(
                        async move { cache::classify_games(&games, &steam_root, steamid3).await },
                        Message::CacheClassified,
                    );

                    let mut tasks: Vec<Task<Message>> = vec![classify_task];
                    if !enumerated.is_empty() {
                        let cached = cache::make_cached_library(
                            enumerated
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
                        tasks.push(Task::perform(
                            async move {
                                cache::write_library_cache(&cached)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            |r| Message::PersistentCacheWritten("library", r),
                        ));
                    }

                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        let scan_task = profile_view::update(pv_state, pv_msg);
                        tasks.push(scan_task);

                        if !app.library_name_map.is_empty() {
                            for game in &mut pv_state.games {
                                if let Some(name) = app.library_name_map.get(&game.app_id) {
                                    game.name = Some(name.clone());
                                }
                            }
                            app.library_name_map.clear();
                        }
                    }
                    return Task::batch(tasks);
                }
                _ => {}
            }

            if let Screen::ProfileView(pv_state) = &mut app.screen {
                return profile_view::update(pv_state, pv_msg);
            }
            Task::none()
        }

        Message::CacheClassified(result) => {
            let ClassifyResult {
                hits,
                dirty,
                schema_bumped,
            } = result;

            app.pending_hit_queue.extend(hits);

            if let Screen::ProfileView(pv_state) = &mut app.screen
                && !dirty.is_empty()
            {
                let total = pv_state.games.len();
                app.messaging.footer = FooterStatus::Scanning {
                    current: 0,
                    total,
                    label: "Loading achievements\u{2026}".to_owned(),
                };
                let mut scanner = crate::progress_scan::ProgressScanner::new(dirty);
                pv_state.progress_rx = scanner.take_receiver();
                pv_state.progress_scanner = Some(scanner);
            }

            if schema_bumped > 0 {
                app.messaging.push_toast(
                    ToastKind::Info,
                    format!("Cache rebuilt: {} entries updated", schema_bumped),
                    None,
                );
            }
            Task::none()
        }

        Message::DrainHitQueue => {
            const HITS_PER_TICK: usize = 8;
            for _ in 0..HITS_PER_TICK {
                let Some(hit) = app.pending_hit_queue.pop_front() else {
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
                }
                app.cached_entries.insert(hit.app_id, entry);
            }
            Task::none()
        }

        Message::CacheWritten { app_id, result } => {
            if let Err(e) = result {
                eprintln!("[steamlens] cache: write failed for app {app_id}: {e}");
            }
            Task::none()
        }

        Message::ClearAllCache => {
            let cache_games_dir = settings::steamlens_root().join("cache").join("games");
            let cache_images_dir = settings::steamlens_root().join("cache").join("images");
            app.cached_entries.clear();
            if let Screen::ProfileView(pv_state) = &mut app.screen {
                for entry in &mut pv_state.games {
                    entry.progress = None;
                }
                let all_ids: Vec<u32> = pv_state.games.iter().map(|g| g.app_id).collect();
                if !all_ids.is_empty() {
                    let mut scanner = crate::progress_scan::ProgressScanner::new(all_ids);
                    pv_state.progress_rx = scanner.take_receiver();
                    pv_state.progress_scanner = Some(scanner);
                }
            }
            Task::batch([Task::perform(
                async move {
                    let _ = tokio::fs::remove_dir_all(&cache_games_dir).await;
                    let _ = tokio::fs::remove_dir_all(&cache_images_dir).await;
                },
                |()| Message::ToastRequest("Cache cleared".to_owned()),
            )])
        }

        Message::ClearGameCache(app_id) => {
            app.cached_entries.remove(&app_id);

            if let Screen::ProfileView(pv_state) = &mut app.screen
                && let Some(entry) = pv_state.games.iter_mut().find(|e| e.app_id == app_id)
            {
                entry.progress = None;
            }
            if let Some(pv_state) = &mut app.profile_view_state
                && let Some(entry) = pv_state.games.iter_mut().find(|e| e.app_id == app_id)
            {
                entry.progress = None;
            }

            let cache_path = cache::game_cache_path(app_id);
            let name = if let Screen::GameView(state) = &app.screen {
                state.game_name.clone()
            } else {
                format!("App {app_id}")
            };
            Task::perform(
                async move {
                    let _ = tokio::fs::remove_file(&cache_path).await;
                    name
                },
                |name| Message::ToastRequest(format!("Cache cleared for {name}")),
            )
        }

        Message::OpenGameView(app_id) => {
            if let Screen::ProfileView(pv_state) = std::mem::replace(
                &mut app.screen,
                Screen::GameView(Box::new(GameViewState::new(app_id))),
            ) {
                app.profile_view_state = Some(pv_state);
            }

            disconnect_worker(app);

            let (worker, rx) = SteamWorker::spawn();
            worker.send(SteamRequest::ConnectWithApp(app_id));

            let mut state = GameViewState::new(app_id);
            state.filter = app.settings.manager.filter;
            state.achievement_sort = app.settings.manager.sort;
            state.rarity_tier_set = app.settings.manager.rarity_tiers.iter().copied().collect();
            state.include_hidden = app.settings.manager.include_hidden;
            state.search_query = app.settings.manager.search.clone();

            if let Some(cached) = app.cached_entries.get(&app_id) {
                seed_game_view_from_cache(&mut state, cached);
            }

            app.worker = Some(worker);
            app.worker_rx = Some(rx);
            app.screen = Screen::GameView(Box::new(state));

            Task::none()
        }

        Message::GameView(m) => {
            let sync_after = matches!(
                &m,
                GameViewMessage::RarityTierToggled(_)
                    | GameViewMessage::RarityFilterCleared
                    | GameViewMessage::HiddenPillToggled
            );

            match &m {
                GameViewMessage::FilterChanged(f) => {
                    app.settings.manager.filter = *f;
                    mark_settings_dirty(app);
                }
                GameViewMessage::AchievementSortChanged(s) => {
                    app.settings.manager.sort = *s;
                    mark_settings_dirty(app);
                }
                GameViewMessage::SearchChanged(q) => {
                    app.settings.manager.search = q.clone();
                    mark_settings_dirty(app);
                }
                _ => {}
            }

            let task = if let Screen::GameView(state) = &mut app.screen
                && let Some(worker) = &app.worker
            {
                game_view::update(state, m, worker)
            } else {
                Task::none()
            };

            if sync_after {
                if let Screen::GameView(state) = &app.screen {
                    app.settings.manager.rarity_tiers =
                        state.rarity_tier_set.iter().copied().collect();
                    app.settings.manager.include_hidden = state.include_hidden;
                }
                mark_settings_dirty(app);
            }

            task
        }

        Message::PollWorker => drain_worker_replies(app),

        Message::DrainProgressResults => {
            let Screen::ProfileView(pv_state) = &mut app.screen else {
                return Task::none();
            };
            let mut tasks: Vec<Task<Message>> = Vec::new();
            if let Some(scanner) = &mut pv_state.progress_scanner {
                let _still_going = scanner.poll();
            }
            if let Some(rx) = &mut pv_state.progress_rx {
                loop {
                    match rx.try_recv() {
                        Ok(result) => {
                            let scan_app_id = result.app_id;
                            let Some(data) = result.data else {
                                pv_state.failed_app_ids.insert(scan_app_id);
                                tasks.push(Task::done(Message::ProfileView(
                                    ProfileViewMessage::ScanFailed(format!(
                                        "Scan failed for app {scan_app_id}"
                                    )),
                                )));
                                continue;
                            };

                            // Record (app_id, change_number) so the next
                            // boot skips this app until its package
                            // advances.
                            if data.achievements.is_empty() {
                                let change_number = pv_state
                                    .games
                                    .iter()
                                    .find(|g| g.app_id == scan_app_id)
                                    .map(|g| g.change_number);
                                pv_state.games.retain(|g| g.app_id != scan_app_id);
                                app.cached_entries.remove(&scan_app_id);
                                if let Some(cn) = change_number {
                                    app.no_ach_cache.insert(scan_app_id, cn);
                                    let snapshot = app.no_ach_cache.clone();
                                    tasks.push(Task::perform(
                                        async move {
                                            cache::write_no_achievements_cache(&snapshot)
                                                .await
                                                .map_err(|e| e.to_string())
                                        },
                                        Message::NoAchCacheWritten,
                                    ));
                                }
                                continue;
                            }

                            {
                                let earned = data.earned_count();
                                let total = data.total_count();

                                tasks.push(Task::done(Message::ProfileView(
                                    ProfileViewMessage::ProgressFetched {
                                        app_id: scan_app_id,
                                        earned,
                                        total,
                                    },
                                )));

                                if let Some(scanned_name) = &data.app_name
                                    && let Some(game) =
                                        pv_state.games.iter_mut().find(|g| g.app_id == scan_app_id)
                                {
                                    game.name = Some(scanned_name.clone());
                                }

                                if let Some(game) =
                                    pv_state.games.iter().find(|g| g.app_id == scan_app_id)
                                {
                                    let entry = build_cache_entry_from_scan(
                                        &data,
                                        scan_app_id,
                                        game.name.as_deref(),
                                        &app.steam_root,
                                        app.steamid3,
                                    );
                                    app.cached_entries.insert(scan_app_id, entry.clone());
                                    tasks.push(Task::perform(
                                        async move {
                                            cache::write_game_cache(&entry)
                                                .await
                                                .map_err(|e| e.to_string())
                                        },
                                        move |res| Message::CacheWritten {
                                            app_id: scan_app_id,
                                            result: res,
                                        },
                                    ));
                                }
                            }
                        }
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                            tasks.push(Task::done(Message::ProfileView(
                                ProfileViewMessage::ProgressScanDone,
                            )));
                            break;
                        }
                    }
                }
            }
            if tasks.is_empty() {
                Task::none()
            } else {
                Task::batch(tasks)
            }
        }

        Message::SettingsFlushTick => {
            if let Some(since) = app.settings_dirty_since
                && since.elapsed() >= Duration::from_millis(200)
            {
                app.settings_dirty_since = None;
                let snapshot = app.settings.clone();
                return Task::perform(
                    async move {
                        settings::write_settings(&snapshot)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::SettingsWritten,
                );
            }
            Task::none()
        }

        Message::SettingsWritten(result) => {
            if let Err(e) = result {
                eprintln!("[steamlens] settings: write error: {e}");
                return Task::done(Message::ToastRequest("Could not save settings".to_owned()));
            }
            Task::none()
        }

        Message::ToastRequest(msg) => {
            app.messaging.push_toast(ToastKind::Info, msg, None);
            Task::none()
        }

        Message::SplashMinElapsed => {
            app.splash_min_elapsed = true;
            Task::none()
        }

        Message::RetrySteamConnect => {
            app.steam_running = None;
            app.splash_scan_done = false;
            app.messaging
                .dismiss_all_banners_by_severity(BannerSeverity::Warning);
            app.messaging.footer = FooterStatus::Scanning {
                current: 0,
                total: 0,
                label: "Connecting to Steam\u{2026}".to_owned(),
            };
            if let Screen::ProfileView(pv_state) = &mut app.screen {
                pv_state.steam_running = None;
            }
            Task::perform(
                async {
                    steamlens_core::probe_steam(timeouts::PROBE_STEAM_RECONNECT)
                        .await
                        .map_err(|e| e.to_string())
                },
                Message::ProbeResult,
            )
        }

        Message::ProbeResult(result) => {
            app.splash_probe_done = true;
            match result {
                Ok(p) => {
                    app.steam_running = Some(true);
                    app.messaging
                        .dismiss_all_banners_by_severity(BannerSeverity::Warning);
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        pv_state.steam_running = Some(true);
                    }
                    let account_name = app
                        .user_profile
                        .as_ref()
                        .map(|u| u.account_name.clone())
                        .unwrap_or_default();
                    app.steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
                    app.profile_avatar_handle = p
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
                    app.steam_level = p.steam_level;
                    app.user_profile = Some(UserProfile {
                        steam_id: p.steam_id,
                        persona_name: p.persona_name,
                        account_name,
                        avatar_png_bytes: p.avatar_image,
                    });

                    let mut tasks: Vec<Task<Message>> = Vec::new();

                    tasks.push(Task::perform(
                        async move {
                            cache::write_profile_cache(&cached)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |r| Message::PersistentCacheWritten("profile", r),
                    ));

                    if !p.game_summaries.is_empty() {
                        app.splash_scan_done = true;
                        let no_ach = &app.no_ach_cache;
                        let filtered: Vec<_> = p
                            .game_summaries
                            .into_iter()
                            .filter(|g| !no_ach.is_known_empty(g.app_id, g.change_number))
                            .collect();
                        let total = filtered.len();
                        app.messaging.footer = FooterStatus::Scanning {
                            current: 0,
                            total,
                            label: "Discovering library\u{2026}".to_owned(),
                        };
                        tasks.push(Task::done(Message::ProfileView(
                            ProfileViewMessage::ScanComplete(filtered),
                        )));
                    } else {
                        tasks.push(Task::perform(
                            async { cache::load_library_cache().await },
                            Message::LibraryCacheLoaded,
                        ));
                    }

                    Task::batch(tasks)
                }
                Err(e) => {
                    app.steam_running = Some(false);
                    app.steam_level = None;
                    app.splash_scan_done = true;
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        pv_state.steam_running = Some(false);
                    }
                    eprintln!("[steamlens] probe failed: {e}");

                    let cached_count = if let Screen::ProfileView(pv) = &app.screen {
                        pv.games.len()
                    } else {
                        0
                    };
                    app.messaging.footer = FooterStatus::Offline {
                        cached_games: cached_count,
                    };
                    app.messaging.push_banner(
                        BannerSeverity::Warning,
                        "Steam is not running \u{2014} showing cached data",
                        Some(messaging::BannerAction {
                            label: "Retry",
                            message: Message::RetrySteamConnect,
                        }),
                        false,
                    );

                    Task::perform(
                        async { cache::load_profile_cache().await },
                        Message::ProfileCacheLoaded,
                    )
                }
            }
        }

        Message::ProfileCacheLoaded(maybe) => {
            let Some(cached) = maybe else {
                return Task::none();
            };
            if app.user_profile.is_some() && app.steam_running != Some(false) {
                return Task::none();
            }
            app.steamid3 = cached.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
            app.steam_level = None;
            app.profile_avatar_handle = cached
                .avatar_png_bytes
                .as_ref()
                .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));
            app.user_profile = Some(UserProfile {
                steam_id: cached.steam_id,
                persona_name: cached.persona_name,
                account_name: cached.account_name,
                avatar_png_bytes: cached.avatar_png_bytes,
            });
            Task::none()
        }

        Message::LibraryCacheLoaded(maybe) => {
            let Some(cached) = maybe else {
                return Task::none();
            };
            let games_present = if let Screen::ProfileView(pv) = &app.screen {
                !pv.games.is_empty()
            } else {
                true
            };
            if games_present {
                return Task::none();
            }
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
            app.library_name_map = name_map;
            Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
                summary,
            )))
        }

        Message::NoAchCacheLoaded(loaded) => {
            app.no_ach_cache = loaded;
            Task::none()
        }

        Message::NoAchCacheWritten(result) => {
            if let Err(e) = result {
                eprintln!("[steamlens] no_achievements cache: write failed: {e}");
            }
            Task::none()
        }

        Message::PersistentCacheWritten(label, result) => {
            if let Err(e) = result {
                eprintln!("[steamlens] {label} cache: write failed: {e}");
            }
            Task::none()
        }

        Message::ToastTick => {
            app.messaging.tick_toasts();
            Task::none()
        }

        Message::ToastHovered(id, hovered) => {
            app.messaging.set_toast_hovered(id, hovered);
            Task::none()
        }

        Message::DismissToast(id) => {
            app.messaging.dismiss_toast(id);
            Task::none()
        }

        Message::DismissBanner(id) => {
            app.messaging.dismiss_banner(id);
            Task::none()
        }

        Message::SkeletonTick => {
            app.skeleton_phase = (app.skeleton_phase + 0.02) % 1.0;
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
                    && let Some(w) = &app.worker
                {
                    return game_view::update(state, GameViewMessage::ApplyChanges, w);
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
            _ => Task::none(),
        },

        Message::ToggleGamePin(app_id) => {
            if let Some(pos) = app
                .settings
                .library
                .pinned
                .iter()
                .position(|&id| id == app_id)
            {
                app.settings.library.pinned.remove(pos);
            } else {
                app.settings.library.pinned.push(app_id);
            }
            mark_settings_dirty(app);
            Task::none()
        }
    }
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
            let mut row = AchievementRow::from_data(data);
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
            let mut row = AchievementRow::from_data(data);
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
            let value = if let Some(i) = s.value_int {
                StatValue::Int(i as i32)
            } else if let Some(f) = s.value_float {
                StatValue::Float(f as f32)
            } else {
                StatValue::Int(0)
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
            StatRow::from_data(data)
        })
        .collect();

    state.phase = GameViewPhase::Connecting;
    state.reveal_queue.clear();
}

fn build_cache_entry_from_scan(
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
            let (value_int, value_float) = match s.value {
                StatValue::Int(i) => (Some(i as i64), None),
                StatValue::Float(f) => (None, Some(f as f64)),
            };
            CachedStat {
                api_name: s.id.clone(),
                display_name: s.display_name.clone(),
                value_int,
                value_float,
                default_value: None,
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
            let mut row = AchievementRow::from_data(data);
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
            let (value_int, value_float) = match r.data.value {
                StatValue::Int(i) => (Some(i as i64), None),
                StatValue::Float(f) => (None, Some(f as f64)),
            };
            CachedStat {
                api_name: r.data.id.clone(),
                display_name: r.data.display_name.clone(),
                value_int,
                value_float,
                default_value: None,
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
    let screen_content: Element<'_, Message> = match &app.screen {
        Screen::ProfileView(pv_state) => profile_view::view_with_cache_actions(
            pv_state,
            app.user_profile.as_ref(),
            app.profile_avatar_handle.as_ref(),
            &app.cached_entries,
            app.skeleton_phase,
            &app.settings.library.pinned,
            app.steam_level,
        ),

        Screen::SteamNotRunning { reason } => {
            let content: Element<'_, Message> = column![
                text("Steam is not running").size(28),
                text("Start the Steam client and try again.").size(16),
                text(reason.as_str()).size(14),
                row![
                    button(text("Back")).on_press(Message::GoBack),
                    button(text("Exit")).on_press(Message::Exit),
                ]
                .spacing(8),
            ]
            .spacing(16)
            .into();
            center(content).into()
        }

        Screen::GameView(state) => game_view::view(state, app.skeleton_phase),
    };

    let failed_count = if let Screen::ProfileView(pv_state) = &app.screen {
        pv_state.failed_app_ids.len()
    } else {
        0
    };
    let with_messaging =
        messaging::wrap_with_messaging(screen_content, &app.messaging, failed_count);

    if app.splash_min_elapsed && app.splash_scan_done && app.splash_probe_done {
        with_messaging
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
        Screen::GameView(state) => matches!(
            state.phase,
            game_view::GameViewPhase::Connecting
                | game_view::GameViewPhase::WaitingStats
                | game_view::GameViewPhase::LoadingData
        ),
        _ => false,
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

    let poll_sub = if app.worker.is_some() {
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::PollWorker)
    } else {
        Subscription::none()
    };

    let game_view_sub = if let Screen::GameView(state) = &app.screen {
        game_view::subscription(state)
    } else {
        Subscription::none()
    };

    let profile_view_spinner_sub = if let Screen::ProfileView(state) = &app.screen {
        if state.is_streaming() {
            iced::time::every(std::time::Duration::from_millis(80))
                .map(|_| Message::ProfileView(ProfileViewMessage::SpinnerTick(0.0)))
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    let progress_sub = if let Screen::ProfileView(state) = &app.screen {
        if state.progress_scanner.is_some() {
            iced::time::every(std::time::Duration::from_millis(200))
                .map(|_| Message::DrainProgressResults)
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    let loader_pulse_sub = if let Screen::ProfileView(state) = &app.screen {
        if state.loader_needs_pulse_subscription() {
            iced::time::every(std::time::Duration::from_millis(70))
                .map(|_| Message::ProfileView(ProfileViewMessage::LoaderPulseTick))
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    let skeleton_sub = if has_active_skeletons(app) {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::SkeletonTick)
    } else {
        Subscription::none()
    };

    let settings_flush_sub = if app.settings_dirty_since.is_some() {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::SettingsFlushTick)
    } else {
        Subscription::none()
    };

    let toast_sub = if app.messaging.has_active_toasts() {
        iced::time::every(Duration::from_millis(500)).map(|_| Message::ToastTick)
    } else {
        Subscription::none()
    };

    let hit_drain_sub = if !app.pending_hit_queue.is_empty() {
        iced::time::every(Duration::from_millis(16)).map(|_| Message::DrainHitQueue)
    } else {
        Subscription::none()
    };

    Subscription::batch([
        keyboard_sub,
        poll_sub,
        game_view_sub,
        profile_view_spinner_sub,
        progress_sub,
        loader_pulse_sub,
        skeleton_sub,
        settings_flush_sub,
        toast_sub,
        hit_drain_sub,
    ])
}

fn theme(_app: &App) -> iced::Theme {
    crate::theme::theme()
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--probe" {
        worker::run_probe();
    }
    if args.len() == 3 && args[1] == "--worker" {
        let app_id: u32 = args[2].parse().unwrap_or_else(|_| {
            eprintln!("steamlens-app: invalid app_id: {}", args[2]);
            std::process::exit(2);
        });
        worker::run(app_id);
    }
    if args.len() >= 2 && args[1].starts_with("--worker") {
        eprintln!("usage: steamlens-app --worker <app_id>");
        std::process::exit(2);
    }

    let swept = steamlens_core::sweep_orphans();
    if swept > 0 {
        eprintln!("[steamlens] swept {swept} orphan shm region(s) at startup");
    }

    let loaded = settings::load_settings();
    let window_w = loaded.ui.window_width;
    let window_h = loaded.ui.window_height;

    iced::application(move || boot_with_settings(loaded.clone()), update, view)
        .title("SteamLens")
        .theme(theme)
        .subscription(subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(window_w.max(896.0), window_h.max(504.0)),
            min_size: Some(iced::Size::new(896.0, 504.0)),
            ..iced::window::Settings::default()
        })
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app_not_running(reason: &str) -> App {
        App {
            screen: Screen::SteamNotRunning {
                reason: reason.to_owned(),
            },
            worker: None,
            worker_rx: None,
            profile_view_state: None,
            settings: Settings::default(),
            settings_dirty_since: None,
            messaging: MessagingCenter::new(),
            cached_entries: HashMap::new(),
            pending_hit_queue: VecDeque::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            profile_avatar_handle: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            splash_probe_done: true,
            steam_running: Some(true),
            steam_level: None,
            skeleton_phase: 0.0,
            no_ach_cache: cache::NoAchievementsCache::new(),
            library_name_map: HashMap::new(),
        }
    }

    fn screen_name(app: &App) -> &'static str {
        match app.screen {
            Screen::ProfileView(_) => "ProfileView",
            Screen::SteamNotRunning { .. } => "SteamNotRunning",
            Screen::GameView(_) => "GameView",
        }
    }

    #[tokio::test]
    async fn boot_starts_in_profile_view() {
        let (app, _task) = boot_with_settings(Settings::default());
        assert!(matches!(app.screen, Screen::ProfileView(_)));
        assert!(app.worker.is_some(), "worker must be spawned immediately");
    }

    #[tokio::test]
    async fn go_back_from_not_running_returns_to_profile_view() {
        let mut app = make_app_not_running("pipe closed");
        let _task = update(&mut app, Message::GoBack);
        assert!(
            matches!(app.screen, Screen::ProfileView(_)),
            "expected ProfileView after GoBack, got {}",
            screen_name(&app)
        );
    }

    fn make_app_probing() -> App {
        let mut app = make_app_not_running("");
        app.screen = Screen::ProfileView(Box::new(ProfileViewState::new()));
        app.splash_min_elapsed = false;
        app.splash_scan_done = false;
        app.splash_probe_done = false;
        app.steam_running = None;
        app.user_profile = None;
        app.profile_avatar_handle = None;
        app
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

        assert_eq!(app.steam_running, Some(true));
        assert!(app.splash_probe_done);
        let profile = app.user_profile.as_ref().expect("profile must be set");
        assert_eq!(profile.persona_name, "TestUser");
        assert_eq!(profile.steam_id, 76561198000000042);
        assert_eq!(app.steamid3, 76561198000000042 - STEAMID64_INDIVIDUAL_MIN);
        assert!(app.profile_avatar_handle.is_some());
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
        app.user_profile = Some(prior.clone());

        let _t = update(
            &mut app,
            Message::ProbeResult(Err("Steam is not running".to_owned())),
        );

        assert_eq!(app.steam_running, Some(false));
        assert!(app.splash_probe_done);
        let profile = app
            .user_profile
            .as_ref()
            .expect("profile must be preserved");
        assert_eq!(profile.persona_name, "DiskFallback");
        assert_eq!(profile.steam_id, 1);
    }

    #[test]
    fn probe_result_err_with_no_prior_profile_keeps_none() {
        let mut app = make_app_probing();
        assert!(app.user_profile.is_none(), "precondition: no prior profile");

        let _t = update(&mut app, Message::ProbeResult(Err("timeout".to_owned())));

        assert_eq!(app.steam_running, Some(false));
        assert!(app.splash_probe_done);
        assert!(
            app.user_profile.is_none(),
            "no profile should remain None on probe error without disk fallback"
        );
    }

    fn splash_visible(app: &App) -> bool {
        !(app.splash_min_elapsed && app.splash_scan_done && app.splash_probe_done)
    }

    #[test]
    fn splash_stays_until_all_three_signals_arrive() {
        let mut app = make_app_probing();
        assert!(splash_visible(&app), "all three pending → splash visible");

        app.splash_min_elapsed = true;
        assert!(splash_visible(&app), "only min-elapsed → splash visible");

        app.splash_scan_done = true;
        assert!(
            splash_visible(&app),
            "min+scan but no probe → splash visible"
        );

        app.splash_probe_done = true;
        assert!(!splash_visible(&app), "all three done → splash hidden");
    }

    #[test]
    fn splash_hidden_only_after_probe_resolves() {
        let mut app = make_app_probing();
        app.splash_min_elapsed = true;
        app.splash_scan_done = true;
        assert!(splash_visible(&app), "missing probe → still visible");

        let _t = update(
            &mut app,
            Message::ProbeResult(Err("Steam is not running".to_owned())),
        );
        assert!(!splash_visible(&app), "probe-Err counts as resolved");
    }

    #[test]
    fn profile_cache_loaded_populates_when_user_profile_is_none() {
        let mut app = make_app_probing();
        app.steam_running = Some(false);
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
        let p = app.user_profile.as_ref().expect("profile must be set");
        assert_eq!(p.persona_name, "FromCache");
        assert_eq!(p.account_name, "cache_login");
        assert_eq!(app.steamid3, 76561198000000042 - STEAMID64_INDIVIDUAL_MIN);
    }

    #[test]
    fn profile_cache_loaded_skipped_when_probe_succeeded_first() {
        let mut app = make_app_probing();
        app.steam_running = Some(true);
        app.user_profile = Some(UserProfile {
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
        let p = app.user_profile.as_ref().unwrap();
        assert_eq!(
            p.persona_name, "LiveFromProbe",
            "probe-Ok profile must not be overwritten by cache"
        );
    }

    #[test]
    fn profile_cache_loaded_none_is_noop() {
        let mut app = make_app_probing();
        app.steam_running = Some(false);
        let _t = update(&mut app, Message::ProfileCacheLoaded(None));
        assert!(app.user_profile.is_none());
        assert_eq!(app.steam_running, Some(false));
    }

    #[test]
    fn library_cache_loaded_some_dispatches_scan_complete_when_games_empty() {
        let mut app = make_app_probing();
        app.steam_running = Some(false);
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
        assert_eq!(app.steam_running, None);
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

        let _t = update(&mut app, Message::DrainProgressResults);

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
        };

        let mut state = ProfileViewState::new();
        state.steam_running = Some(true);
        state.games.push(mk_entry(1, true));
        state.games.push(mk_entry(2, true));
        state.games.push(mk_entry(3, false));
        state.failed_app_ids.insert(3);

        assert_eq!(
            state.loader_phase(),
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
        let mut state = ProfileViewState::new();
        state.steam_running = Some(false);
        assert_eq!(state.loader_phase(), LoaderPhase::SteamOff);
    }

    #[test]
    fn loader_phase_alpha_when_no_games_and_steam_unknown() {
        use crate::profile_view::types::{LoaderPhase, ProfileViewState};
        let state = ProfileViewState::new();
        assert_eq!(
            state.loader_phase(),
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
    fn retry_steam_connect_resets_steam_running_and_pv_mirror() {
        let mut app = make_app_probing();
        app.steam_running = Some(false);
        if let Screen::ProfileView(pv) = &mut app.screen {
            pv.steam_running = Some(false);
        }

        let _t = update(&mut app, Message::RetrySteamConnect);

        assert_eq!(
            app.steam_running, None,
            "App.steam_running reset to None during re-probe"
        );
        if let Screen::ProfileView(pv) = &app.screen {
            assert_eq!(pv.steam_running, None, "pv mirror also reset");
        }
    }

    #[test]
    fn account_name_preserved_when_probe_succeeds() {
        let mut app = make_app_probing();
        app.user_profile = Some(UserProfile {
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

        let p = app.user_profile.as_ref().unwrap();
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
        let mut row = AchievementRow::from_data(data);
        row.is_dirty = true;
        state.achievements.push(row);
        state
    }

    #[test]
    fn cancel_resets_dirty_count_to_zero() {
        use game_view::GameViewMessage;
        use steam_worker::SteamWorker;

        let mut state = make_game_view_state_with_dirty_achievements();
        assert_eq!(
            state.dirty_count(),
            1,
            "precondition: one dirty achievement"
        );

        let worker = SteamWorker::new_disconnected();
        let _task = game_view::update(&mut state, GameViewMessage::DiscardChanges, &worker);

        assert_eq!(
            state.dirty_count(),
            0,
            "dirty count must be zero after DiscardChanges"
        );
    }

    #[test]
    fn cancel_does_not_change_phase() {
        use game_view::GameViewMessage;
        use steam_worker::SteamWorker;

        let mut state = make_game_view_state_with_dirty_achievements();
        assert_eq!(state.phase, game_view::GameViewPhase::Ready, "precondition");

        let worker = SteamWorker::new_disconnected();
        let _task = game_view::update(&mut state, GameViewMessage::DiscardChanges, &worker);

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
        use steam_worker::SteamWorker;

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
        state.achievements.push(AchievementRow::from_data(data));
        assert!(
            !state.achievements[0].revealed,
            "precondition: revealed must be false"
        );

        let worker = SteamWorker::new_disconnected();
        let _task = update(
            &mut state,
            GameViewMessage::RevealHidden("ACH_SECRET".to_owned()),
            &worker,
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
            let mut r = AchievementRow::from_data(AchievementData {
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

        let mut zebra = AchievementRow::from_data(AchievementData {
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

        let mut ant = AchievementRow::from_data(AchievementData {
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
            let mut r = AchievementRow::from_data(AchievementData {
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
        use steam_worker::SteamWorker;

        let mut state = GameViewState::new(105600);
        state.phase = GameViewPhase::Ready;
        state.game_name = "Terraria".to_owned();

        let worker = SteamWorker::new_disconnected();
        let _task = update(&mut state, GameViewMessage::ResetClicked, &worker);
        assert!(state.show_reset_modal, "modal must open on ResetClicked");
        assert!(
            state.reset_confirm_input.is_empty(),
            "input must be cleared on open"
        );

        let _task = update(
            &mut state,
            GameViewMessage::ResetConfirmInputChanged("Wrong".to_owned()),
            &worker,
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
        use steam_worker::SteamWorker;

        let mut state = GameViewState::new(105600);
        state.phase = GameViewPhase::Ready;
        state.game_name = "Terraria".to_owned();

        let worker = SteamWorker::new_disconnected();
        let _task = update(&mut state, GameViewMessage::ResetClicked, &worker);

        let _task = update(
            &mut state,
            GameViewMessage::ResetConfirmInputChanged("TERRARIA ".to_owned()),
            &worker,
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
            AchievementRow::from_data(AchievementData {
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
        let mut row = AchievementRow::from_data(data.clone());
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

    #[tokio::test]
    async fn clear_game_cache_removes_cached_entry_and_clears_progress() {
        use profile_view::types::{CapsuleAsset, GameEntry};

        let app_id: u32 = 440;

        let game_entry = GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some("Team Fortress 2".to_owned()),
            capsule: CapsuleAsset::Unavailable,
            progress: Some(crate::progress_scan::ProgressData {
                earned: 10,
                total: 520,
            }),
        };

        let mut pv_state = ProfileViewState::new();
        pv_state.games.push(game_entry);

        let mut app = App {
            screen: Screen::ProfileView(Box::new(pv_state)),
            worker: None,
            worker_rx: None,
            profile_view_state: None,
            settings: Settings::default(),
            settings_dirty_since: None,
            messaging: MessagingCenter::new(),
            cached_entries: {
                let mut m = HashMap::new();
                m.insert(
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
                m
            },
            pending_hit_queue: VecDeque::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            profile_avatar_handle: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            splash_probe_done: true,
            steam_running: Some(true),
            steam_level: None,
            skeleton_phase: 0.0,
            no_ach_cache: cache::NoAchievementsCache::new(),
            library_name_map: HashMap::new(),
        };

        let _task = update(&mut app, Message::ClearGameCache(app_id));

        assert!(
            !app.cached_entries.contains_key(&app_id),
            "cached_entries must no longer contain the cleared app_id"
        );

        if let Screen::ProfileView(pv_state) = &app.screen {
            let entry = pv_state.games.iter().find(|e| e.app_id == app_id);
            assert!(
                entry.is_some(),
                "GameEntry must still exist in profile view"
            );
            assert!(
                entry.unwrap().progress.is_none(),
                "progress must be None after ClearGameCache"
            );
        } else {
            panic!("expected ProfileView screen");
        }
    }

    fn make_app_with_game_view_phase(phase: game_view::GameViewPhase) -> App {
        let mut state = GameViewState::new(570);
        state.phase = phase;
        App {
            screen: Screen::GameView(Box::new(state)),
            worker: None,
            worker_rx: None,
            profile_view_state: None,
            settings: Settings::default(),
            settings_dirty_since: None,
            messaging: MessagingCenter::new(),
            cached_entries: HashMap::new(),
            pending_hit_queue: VecDeque::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            profile_avatar_handle: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            splash_probe_done: true,
            steam_running: Some(true),
            steam_level: None,
            skeleton_phase: 0.0,
            no_ach_cache: cache::NoAchievementsCache::new(),
            library_name_map: HashMap::new(),
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

    #[test]
    fn has_active_skeletons_false_for_not_running_screen() {
        let app = make_app_not_running("Steam not running");
        assert!(
            !has_active_skeletons(&app),
            "SteamNotRunning screen must not trigger skeletons"
        );
    }
}
