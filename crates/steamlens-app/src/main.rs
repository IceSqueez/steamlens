mod cache;
mod capsule_cache;
mod game_view;
mod profile_view;
mod progress_scan;
mod settings;
mod skeleton;
mod steam_worker;
mod theme;
mod worker;

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use iced::keyboard;
use iced::widget::{button, center, column, container, row, text};
use iced::{Alignment, Color, Element, Length, Subscription, Task};

use cache::{ClassifyResult, GameCacheEntry};
use game_view::{GameViewMessage, GameViewState};
use profile_view::types::{ProfileViewMessage, ProfileViewState};
use settings::Settings;
use steam_worker::{SteamReply, SteamRequest, SteamWorker};
use steamlens_core::UserProfile;

#[derive(Debug)]
enum Screen {
    ProfileView(Box<ProfileViewState>),
    SteamNotRunning { reason: String },
    GameView(Box<GameViewState>),
}

struct ToastState {
    message: String,
    expires_at: Instant,
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
    SettingsFlushTick,
    SettingsWritten(Result<(), String>),
    ToastRequest(String),
    ToastTick,
    CacheClassified(ClassifyResult),
    CacheWritten {
        app_id: u32,
        result: Result<(), String>,
    },
    #[allow(dead_code)]
    ClearAllCache,
    #[allow(dead_code)]
    ClearGameCache(u32),
    SkeletonTick,
    #[allow(dead_code)]
    FocusLibrarySearch,
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
    toast: Option<ToastState>,
    /// Full cache entries keyed by app_id.  Populated after classify_games
    /// completes.  Used to seed GameViewState on open (Rule R scaffold).
    cached_entries: HashMap<u32, GameCacheEntry>,
    /// Steam root path cached at boot for use in cache write helpers.
    steam_root: std::path::PathBuf,
    /// SteamID3 cached at boot; 0 when profile load fails.
    steamid3: u64,
    /// Local Steam user profile (persona name + avatar). Loaded once at boot.
    user_profile: Option<UserProfile>,
    /// Splash overlay stays visible until BOTH the 750 ms minimum has elapsed
    /// AND the library scan has reported back (`ScanComplete` or `ScanFailed`).
    /// Whichever takes longer wins — splash is a branded handover for the
    /// actual load, not a cosmetic delay.
    splash_min_elapsed: bool,
    splash_scan_done: bool,
    skeleton_phase: f32,
}

fn boot() -> (App, Task<Message>) {
    let loaded_settings = settings::load_settings();

    let steam_root = settings::default_steam_root();
    let profile_result = steamlens_core::load_local_profile();
    let steamid3 = profile_result
        .as_ref()
        .ok()
        .map(|p| p.steam_id.saturating_sub(76_561_197_960_265_728))
        .unwrap_or(0);

    let mut pv_state = ProfileViewState::new();
    pv_state.sort = loaded_settings.library.sort;
    pv_state.search = loaded_settings.library.search.clone();

    let (worker, rx) = SteamWorker::spawn();
    profile_view::trigger_scan(&worker);

    let app = App {
        screen: Screen::ProfileView(Box::new(pv_state)),
        worker: Some(worker),
        worker_rx: Some(rx),
        profile_view_state: None,
        settings: loaded_settings,
        settings_dirty_since: None,
        toast: None,
        cached_entries: HashMap::new(),
        steam_root,
        steamid3,
        user_profile: profile_result.ok(),
        splash_min_elapsed: false,
        splash_scan_done: false,
        skeleton_phase: 0.0,
    };

    let min_splash_task = Task::perform(
        async { tokio::time::sleep(std::time::Duration::from_millis(750)).await },
        |_| Message::SplashMinElapsed,
    );

    (app, min_splash_task)
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
            if let Some(w) = &app.worker {
                w.send(SteamRequest::Disconnect);
            }
            app.worker = None;
            app.worker_rx = None;
            return Task::none();
        }

        match reply {
            SteamReply::LibraryScan(games) => {
                tasks.push(Task::done(Message::ProfileView(
                    ProfileViewMessage::ScanComplete(games),
                )));
                continue;
            }
            SteamReply::LibraryScanFailed(reason) => {
                tasks.push(Task::done(Message::ProfileView(
                    ProfileViewMessage::ScanFailed(reason),
                )));
                continue;
            }
            _ => {}
        }

        if let SteamReply::Connected { .. } = &reply
            && let Some(w) = &app.worker
        {
            w.send(SteamRequest::RequestUserStats);
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

                    if let Some(w) = &app.worker {
                        w.send(SteamRequest::Disconnect);
                    }
                    app.worker = None;
                    app.worker_rx = None;

                    if let Some(stored) = app.profile_view_state.take() {
                        app.screen = Screen::ProfileView(stored);
                    } else {
                        let pv_state = ProfileViewState::new();
                        let (worker, rx) = SteamWorker::spawn();
                        profile_view::trigger_scan(&worker);
                        app.worker = Some(worker);
                        app.worker_rx = Some(rx);
                        app.screen = Screen::ProfileView(Box::new(pv_state));
                    }
                    return write_task;
                }
                Screen::SteamNotRunning { .. } => {
                    if let Some(w) = &app.worker {
                        w.send(SteamRequest::Disconnect);
                    }
                    app.worker = None;
                    app.worker_rx = None;

                    if let Some(stored) = app.profile_view_state.take() {
                        app.screen = Screen::ProfileView(stored);
                    } else {
                        let pv_state = ProfileViewState::new();
                        let (worker, rx) = SteamWorker::spawn();
                        profile_view::trigger_scan(&worker);
                        app.worker = Some(worker);
                        app.worker_rx = Some(rx);
                        app.screen = Screen::ProfileView(Box::new(pv_state));
                    }
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
                ProfileViewMessage::ManualAppIdSubmitted => {
                    let app_id: u32 = if let Screen::ProfileView(pv_state) = &app.screen {
                        pv_state.manual_app_id_input.parse().unwrap_or(0)
                    } else {
                        0
                    };
                    if app_id == 0 {
                        return Task::none();
                    }
                    return update(app, Message::OpenGameView(app_id));
                }
                ProfileViewMessage::RescanRequested => {
                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        let t = profile_view::update(pv_state, pv_msg);
                        if let Some(w) = &app.worker {
                            profile_view::trigger_scan(w);
                        }
                        return t;
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
                ProfileViewMessage::ScanFailed(_) => {
                    app.splash_scan_done = true;
                }
                ProfileViewMessage::ScanComplete(summaries) => {
                    app.splash_scan_done = true;
                    let games = summaries.clone();
                    let steam_root = app.steam_root.clone();
                    let steamid3 = app.steamid3;
                    let classify_task = Task::perform(
                        async move { cache::classify_games(&games, &steam_root, steamid3).await },
                        Message::CacheClassified,
                    );

                    if let Screen::ProfileView(pv_state) = &mut app.screen {
                        let scan_task = profile_view::update(pv_state, pv_msg);
                        return Task::batch([scan_task, classify_task]);
                    }
                    return classify_task;
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

            if let Screen::ProfileView(pv_state) = &mut app.screen {
                for hit in &hits {
                    if let Some(entry) = pv_state
                        .games
                        .iter_mut()
                        .find(|g| g.summary.app_id == hit.app_id)
                    {
                        use crate::progress_scan::ProgressData;
                        entry.progress = Some(ProgressData {
                            earned: hit.entry.progress.earned,
                            total: hit.entry.progress.total,
                        });
                    }
                    app.cached_entries.insert(hit.app_id, hit.entry.clone());
                }

                if !dirty.is_empty() {
                    let mut scanner = crate::progress_scan::ProgressScanner::new(dirty);
                    pv_state.progress_rx = scanner.take_receiver();
                    pv_state.progress_scanner = Some(scanner);
                }
            } else {
                for hit in &hits {
                    app.cached_entries.insert(hit.app_id, hit.entry.clone());
                }
            }

            if schema_bumped > 0 {
                return Task::done(Message::ToastRequest(format!(
                    "Cache rebuilt: {} entries updated",
                    schema_bumped
                )));
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
                let all_ids: Vec<u32> = pv_state.games.iter().map(|g| g.summary.app_id).collect();
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
                && let Some(entry) = pv_state
                    .games
                    .iter_mut()
                    .find(|e| e.summary.app_id == app_id)
            {
                entry.progress = None;
            }
            if let Some(pv_state) = &mut app.profile_view_state
                && let Some(entry) = pv_state
                    .games
                    .iter_mut()
                    .find(|e| e.summary.app_id == app_id)
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
            let steam_last_updated = if let Screen::ProfileView(pv_state) = &app.screen {
                pv_state
                    .games
                    .iter()
                    .find(|g| g.summary.app_id == app_id)
                    .map(|g| g.summary.last_updated)
                    .unwrap_or(0)
            } else {
                0
            };

            if let Screen::ProfileView(pv_state) = std::mem::replace(
                &mut app.screen,
                Screen::SteamNotRunning {
                    reason: String::new(),
                },
            ) {
                app.profile_view_state = Some(pv_state);
            }

            if let Some(w) = &app.worker {
                w.send(SteamRequest::Disconnect);
            }
            app.worker = None;
            app.worker_rx = None;

            let (worker, rx) = SteamWorker::spawn();
            worker.send(SteamRequest::ConnectWithApp(app_id));

            let mut state = GameViewState::new(app_id);
            state.steam_last_updated = steam_last_updated;
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
                            if let Some(data) = result.data {
                                let scan_app_id = result.app_id;
                                tasks.push(Task::done(Message::ProfileView(
                                    ProfileViewMessage::ProgressFetched {
                                        app_id: scan_app_id,
                                        earned: data.earned,
                                        total: data.total,
                                    },
                                )));

                                if let Some(game) = pv_state
                                    .games
                                    .iter()
                                    .find(|g| g.summary.app_id == scan_app_id)
                                {
                                    let entry = cache::invalidate::make_progress_cache_entry(
                                        &game.summary,
                                        data.earned,
                                        data.total,
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
                match toml::to_string_pretty(&app.settings) {
                    Ok(text) => {
                        let path = settings::settings_path();
                        let bytes = text.into_bytes();
                        return Task::perform(
                            async move {
                                cache::store::atomic_write(&path, &bytes)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            Message::SettingsWritten,
                        );
                    }
                    Err(e) => {
                        eprintln!("[steamlens] settings: serialize error: {e}");
                        return Task::done(Message::ToastRequest(
                            "Could not save settings".to_owned(),
                        ));
                    }
                }
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
            app.toast = Some(ToastState {
                message: msg,
                expires_at: Instant::now() + Duration::from_secs(4),
            });
            Task::none()
        }

        Message::SplashMinElapsed => {
            app.splash_min_elapsed = true;
            Task::none()
        }

        Message::ToastTick => {
            if let Some(toast) = &app.toast
                && Instant::now() >= toast.expires_at
            {
                app.toast = None;
            }
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
                if modifiers.command()
                    && c.as_str() == "k"
                    && matches!(app.screen, Screen::ProfileView(_))
                {
                    return iced::widget::operation::focus(profile_view::library_search_id());
                }
            }
            Task::none()
        }

        Message::FocusLibrarySearch => {
            if matches!(app.screen, Screen::ProfileView(_)) {
                iced::widget::operation::focus(profile_view::library_search_id())
            } else {
                Task::none()
            }
        }
    }
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
        steam_last_updated: state.steam_last_updated,
        steam_last_played,
        cached_at,
        achievements,
        stats,
        progress: CachedProgress { earned, total },
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let screen_content: Element<'_, Message> = match &app.screen {
        Screen::ProfileView(pv_state) => profile_view::view_with_cache_actions(
            pv_state,
            app.user_profile.as_ref(),
            &app.cached_entries,
            app.skeleton_phase,
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

    let with_toast = if let Some(toast) = &app.toast {
        toast_overlay(screen_content, &toast.message)
    } else {
        screen_content
    };

    if app.splash_min_elapsed && app.splash_scan_done {
        with_toast
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

fn toast_overlay<'a>(content: Element<'a, Message>, message: &'a str) -> Element<'a, Message> {
    use iced::widget::{Space, stack};

    let toast_widget = container(
        text(message)
            .size(14)
            .color(Color::from_rgb(0.95, 0.95, 0.95)),
    )
    .padding(iced::Padding::default().top(8).bottom(8).left(16).right(16))
    .style(|_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.18, 0.18, 0.22, 0.92,
        ))),
        border: iced::Border {
            color: Color::from_rgba(0.6, 0.4, 0.9, 0.7),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    let toast_row = row![
        Space::new().width(Length::Fill),
        toast_widget,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let overlay_col = column![
        Space::new().height(Length::Fill),
        container(toast_row)
            .width(Length::Fill)
            .padding(iced::Padding::default().bottom(24)),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    stack![content, overlay_col].into()
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

    let settings_flush_sub =
        iced::time::every(Duration::from_millis(200)).map(|_| Message::SettingsFlushTick);

    let toast_sub = if app.toast.is_some() {
        iced::time::every(Duration::from_millis(500)).map(|_| Message::ToastTick)
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
    ])
}

fn theme(_app: &App) -> iced::Theme {
    crate::theme::theme()
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
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
    let loaded = settings::load_settings();
    let window_w = loaded.ui.window_width;
    let window_h = loaded.ui.window_height;

    iced::application(boot, update, view)
        .title("SteamLens")
        .theme(theme)
        .subscription(subscription)
        .window_size(iced::Size::new(window_w, window_h))
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
            toast: None,
            cached_entries: HashMap::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            skeleton_phase: 0.0,
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
        let (app, _task) = boot();
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
                steam_id: 0,
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
        let _task = handle_steam_reply(
            &mut state,
            SteamReply::Connected {
                steam_id: 0,
                app_name: None,
            },
        );
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

    #[test]
    fn build_game_view_cache_entry_preserves_steam_last_updated() {
        let mut state = GameViewState::new(570);
        state.steam_last_updated = 12345;

        let entry = build_game_view_cache_entry(
            &state,
            570,
            std::path::Path::new("/nonexistent/steam/root"),
            0,
        );

        assert_eq!(
            entry.steam_last_updated, 12345,
            "cache entry must carry the steam_last_updated from GameViewState"
        );
    }

    #[tokio::test]
    async fn clear_game_cache_removes_cached_entry_and_clears_progress() {
        use profile_view::types::{CapsuleAsset, GameEntry};
        use steamlens_core::GameSummary;

        let app_id: u32 = 440;

        let summary = GameSummary {
            app_id,
            name: "Team Fortress 2".to_owned(),
            last_played: None,
            achievement_count: 520,
            last_updated: 9_999,
            manifest_path: std::path::PathBuf::from("/nonexistent"),
        };
        let game_entry = GameEntry {
            summary,
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
            toast: None,
            cached_entries: {
                let mut m = HashMap::new();
                m.insert(
                    app_id,
                    cache::GameCacheEntry {
                        schema_version: cache::CURRENT_SCHEMA_VERSION,
                        app_id,
                        name: "Team Fortress 2".to_owned(),
                        steam_last_updated: 9_999,
                        steam_last_played: 0,
                        cached_at: 1_000,
                        achievements: vec![],
                        stats: vec![],
                        progress: cache::types::CachedProgress {
                            earned: 10,
                            total: 520,
                        },
                    },
                );
                m
            },
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            skeleton_phase: 0.0,
        };

        let _task = update(&mut app, Message::ClearGameCache(app_id));

        assert!(
            !app.cached_entries.contains_key(&app_id),
            "cached_entries must no longer contain the cleared app_id"
        );

        if let Screen::ProfileView(pv_state) = &app.screen {
            let entry = pv_state.games.iter().find(|e| e.summary.app_id == app_id);
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
            toast: None,
            cached_entries: HashMap::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            splash_min_elapsed: true,
            splash_scan_done: true,
            skeleton_phase: 0.0,
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
