#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_context;
mod boot;
mod cache;
mod capsule_cache;
mod capsule_commands;
mod game_cache_builder;
mod game_view;
mod ipc_pipe;
mod logging;
mod messaging;
mod paths;
mod profile_view;
mod progress_scan;
mod routing;
mod screen;
mod settings;
mod settings_commands;
mod splash;
mod splash_commands;
mod steam_connectivity;
mod steam_worker;
mod timeouts;
mod ui;
mod update_check;
mod update_handlers;
mod worker;
mod worker_drain;
mod worker_subprocess;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::process::{self, Command};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use iced::futures::SinkExt;
use iced::futures::channel::mpsc as iced_mpsc;
use iced::keyboard;
use iced::widget::column;
use iced::{Element, Subscription, Task};

use app_context::AppContext;
use game_view::{GameViewMessage, GameViewState};
use profile_view::types::{ProfileViewMessage, ProfileViewState};
use steamlens_core::{ProbeError, ProbedProfile};

#[derive(Debug)]
pub(crate) enum Screen {
    ProfileView(Box<ProfileViewState>),
    GameView(Box<GameViewState>),
}

#[derive(Debug, Clone)]
pub(crate) enum ProbeFailure {
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
pub(crate) enum Message {
    DiscardReply,
    GoBack,
    ProfileView(ProfileViewMessage),
    GameView(GameViewMessage),
    Cache(cache::CacheEvent),
    Messaging(messaging::MessagingEvent),
    WorkerReply(steam_worker::WorkerReply),
    KeyboardEvent(keyboard::Event),
    SplashMinElapsed,
    ProbeResult(Result<Box<ProbedProfile>, ProbeFailure>),
    ProbeLibraryReady {
        account_id: u32,
        summaries: Vec<steamlens_core::GameSummary>,
        no_ach: cache::NoAchievementsCache,
    },
    RetrySteamConnect,
    SettingsFlushTick,
    SettingsWritten(Result<(), String>),
    DrainHitQueue,
    AnimationFrame(Instant),
    FocusSearch,
    GlobalSearchChanged(String),
    PersistGameSummary(u32),
    InvalidateGameCache(u32),
    ShowAbout,
    DismissAbout,
    OpenUrl(String),
    ToggleTheme,
    UpdateCheckResult(Result<Option<update_check::UpdateInfo>, String>),
    SteamStateRefreshed(
        Option<(
            HashMap<u32, steamlens_core::SteamAppState>,
            Option<SystemTime>,
        )>,
    ),
    LocalProfileLoaded(Option<Box<steamlens_core::UserProfile>>),
    AppAssetsLoaded(HashMap<u32, steamlens_core::AppLibraryAssets>),
}

struct WorkerReplyHandle {
    generation: u64,
    reply_receiver: steam_worker::SharedWorkerReplyReceiver,
}

impl Hash for WorkerReplyHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.generation.hash(state);
    }
}

struct ProgressRxHandle {
    rx: profile_view::types::SharedProgressRx,
    generation: u64,
}

impl Hash for ProgressRxHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.generation.hash(state);
    }
}

#[derive(Default)]
struct BootStage {
    splash_min_elapsed: bool,
    library_cache_resolved: bool,
    cache_classified: bool,
    probe_done: bool,
    probe_classified: bool,
}

impl BootStage {
    fn is_ready(&self) -> bool {
        self.splash_min_elapsed
            && self.library_cache_resolved
            && self.cache_classified
            && self.probe_done
    }
}

#[derive(Default)]
struct Modals {
    about_open: bool,
}

pub(crate) struct App {
    pub(crate) context: AppContext,
    pub(crate) screen: Screen,
    pub(crate) preserved_profile_state: Option<Box<ProfileViewState>>,
    pub(crate) boot: BootStage,
    pub(crate) modals: Modals,
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::DiscardReply => Task::none(),

        Message::GoBack => match &app.screen {
            Screen::GameView(_) => Task::done(Message::GameView(GameViewMessage::GoBackRequested)),
            _ => Task::none(),
        },

        Message::ProfileView(msg) => update_handlers::handle_profile_view(app, msg),

        Message::Cache(cache::CacheEvent::Classified(result)) => {
            update_handlers::handle_cache_classified(app, result)
        }

        Message::DrainHitQueue => update_handlers::handle_drain_hit_queue(app),

        Message::Cache(cache::CacheEvent::GameWritten { app_id, result }) => {
            update_handlers::handle_game_written(app_id, result)
        }

        Message::PersistGameSummary(app_id) => {
            update_handlers::handle_persist_game_summary(app, app_id)
        }

        Message::InvalidateGameCache(app_id) => {
            update_handlers::handle_invalidate_game_cache(app, app_id)
        }

        Message::Cache(cache::CacheEvent::GameInvalidated {
            app_id,
            name,
            result,
        }) => update_handlers::handle_game_invalidated(app, app_id, name, result),

        Message::GameView(message) => update_handlers::handle_game_view_message(app, message),

        Message::WorkerReply(reply) => worker_drain::handle_worker_reply(app, reply),

        Message::SettingsFlushTick => update_handlers::handle_settings_flush_tick(app),

        Message::SettingsWritten(result) => update_handlers::handle_settings_written(app, result),

        Message::SplashMinElapsed => {
            app.boot.splash_min_elapsed = true;
            Task::none()
        }

        Message::RetrySteamConnect => update_handlers::handle_retry_steam_connect(app),

        Message::ProbeResult(result) => update_handlers::handle_probe_result(app, result),

        Message::ProbeLibraryReady {
            account_id,
            summaries,
            no_ach,
        } => update_handlers::handle_probe_library_ready(app, account_id, summaries, no_ach),

        Message::Cache(cache::CacheEvent::ProfileLoaded(cached)) => {
            update_handlers::handle_profile_loaded(app, cached)
        }

        Message::Cache(cache::CacheEvent::LibraryLoaded(cached)) => {
            update_handlers::handle_library_loaded(app, cached)
        }

        Message::Cache(cache::CacheEvent::NoAchievementsWritten(result)) => {
            update_handlers::handle_no_ach_written(result)
        }

        Message::Cache(cache::CacheEvent::PersistentWritten(label, result)) => {
            update_handlers::handle_persistent_written(app, label, result)
        }

        Message::Messaging(msg) => update_handlers::handle_messaging(app, msg),

        Message::AnimationFrame(now) => update_handlers::handle_animation_frame(app, now),

        Message::KeyboardEvent(event) => update_handlers::handle_keyboard_event(app, event),

        Message::FocusSearch => update_handlers::handle_focus_search(app),

        Message::GlobalSearchChanged(query) => {
            update_handlers::handle_global_search_changed(app, query)
        }

        Message::ShowAbout => {
            app.modals.about_open = true;
            Task::none()
        }

        Message::DismissAbout => {
            app.modals.about_open = false;
            Task::none()
        }

        Message::OpenUrl(url) => {
            open_url_in_browser(&url);
            Task::none()
        }

        Message::ToggleTheme => update_handlers::handle_toggle_theme(app),

        Message::UpdateCheckResult(result) => {
            update_handlers::handle_update_check_result(app, result)
        }

        Message::SteamStateRefreshed(payload) => {
            update_handlers::handle_steam_state_refreshed(app, payload)
        }

        Message::AppAssetsLoaded(assets) => update_handlers::handle_app_assets_loaded(app, assets),

        Message::LocalProfileLoaded(profile) => {
            update_handlers::handle_local_profile_loaded(app, profile)
        }

        Message::Cache(cache::CacheEvent::OfflineLoaded { app_id, entry }) => {
            update_handlers::handle_offline_loaded(app, app_id, entry)
        }
    }
}

fn open_url_in_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let cmd = Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let cmd = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let cmd = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    if let Err(e) = cmd {
        tracing::warn!("failed to open url {url}: {e}");
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let skeleton_phase = app.context.animation.skeleton_phase;

    let theme = app.context.settings.ui.theme;
    let header: Option<Element<'_, Message>> = match &app.screen {
        Screen::ProfileView(profile_view_state) => Some(crate::screen::render_app_header(
            profile_view::header_content(
                profile_view_state,
                app.context.connectivity.steam_running,
                theme,
            ),
        )),
        Screen::GameView(state) => Some(crate::screen::render_app_header(
            game_view::header_content(state, theme),
        )),
    };

    let body: Element<'_, Message> = match &app.screen {
        Screen::ProfileView(profile_view_state) => {
            let props = profile_view::ProfileViewProps {
                user_profile: app.context.user.profile.as_ref(),
                avatar_handle: app.context.user.avatar_handle.as_ref(),
                cached_entries: &app.context.game_cache.entries,
                capsules: &app.context.capsules,
                skeleton_phase,
                pinned: &app.context.settings.library.pinned,
                steam_level: app.context.user.steam_level,
                steam_running: app.context.connectivity.steam_running,
                theme,
            };
            crate::screen::compose_screen(profile_view::render(profile_view_state, props))
                .map(Message::ProfileView)
        }

        Screen::GameView(state) => {
            let props = game_view::GameViewProps {
                skeleton_phase,
                app_theme: theme,
                capsules: &app.context.capsules,
            };
            game_view::view(state, props).map(Message::GameView)
        }
    };

    let banner_slot = messaging::banner_stack(&app.context.messaging);

    let mut shell = column![].spacing(0);
    if let Some(header) = header {
        shell = shell.push(header);
    }
    if let Some(banner) = banner_slot {
        shell = shell.push(banner);
    }
    shell = shell.push(body);
    let shell: Element<'_, Message> = shell.into();

    let with_toasts = messaging::wrap_with_toasts(shell, &app.context.messaging);

    let ready = app.boot.is_ready();
    let base = if ready {
        with_toasts
    } else {
        splash::splash_view(splash::splash_status_text(app))
    };

    if app.modals.about_open {
        let modal = ui::about_modal::about_modal(
            Message::DismissAbout,
            Message::OpenUrl("https://github.com/IceSqueez/steamlens".to_owned()),
            Message::OpenUrl("https://github.com/IceSqueez/steamlens/issues".to_owned()),
            Message::OpenUrl("https://github.com/IceSqueez/steamlens/releases".to_owned()),
            app.context.settings.ui.theme,
        );
        iced::widget::stack![base, modal].into()
    } else {
        base
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

    let worker_reply_sub = match &app.context.worker.current {
        Some(worker) => Subscription::run_with(
            WorkerReplyHandle {
                generation: worker.generation(),
                reply_receiver: worker.reply_receiver(),
            },
            |handle: &WorkerReplyHandle| {
                let receiver_holder = Arc::clone(&handle.reply_receiver);
                iced::stream::channel(64, |mut output: iced_mpsc::Sender<Message>| async move {
                    let Some(mut receiver) = receiver_holder
                        .lock()
                        .expect("worker_reply_receiver poisoned")
                        .take()
                    else {
                        return;
                    };
                    while let Some(reply) = receiver.recv().await {
                        if output.send(Message::WorkerReply(reply)).await.is_err() {
                            break;
                        }
                    }
                })
            },
        ),
        None => Subscription::none(),
    };

    let animation_sub = if update_handlers::needs_animation_frame(app) {
        iced::time::every(Duration::from_millis(33)).map(Message::AnimationFrame)
    } else {
        Subscription::none()
    };

    let settings_flush_sub = if app.context.settings_dirty_since.is_some() {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::SettingsFlushTick)
    } else {
        Subscription::none()
    };

    let toast_sub = if app.context.messaging.has_active_toasts() {
        iced::time::every(Duration::from_millis(500))
            .map(|_| Message::Messaging(messaging::MessagingEvent::ToastTick))
    } else {
        Subscription::none()
    };

    let hit_drain_sub = if !app.context.game_cache.pending_hits.is_empty() {
        iced::time::every(Duration::from_millis(33)).map(|_| Message::DrainHitQueue)
    } else {
        Subscription::none()
    };

    let profile_progress_sub =
        match routing::current_profile_view_state(&app.screen, &app.preserved_profile_state) {
            Some(state) if state.progress_scanner.is_some() => Subscription::run_with(
                ProgressRxHandle {
                    rx: Arc::clone(&state.progress_rx),
                    generation: state.scan_generation,
                },
                |handle: &ProgressRxHandle| {
                    let rx_holder = Arc::clone(&handle.rx);
                    let generation = handle.generation;
                    iced::stream::channel(
                        64,
                        move |mut output: iced_mpsc::Sender<Message>| async move {
                            let Some(mut rx) =
                                rx_holder.lock().expect("progress_rx poisoned").take()
                            else {
                                return;
                            };
                            while let Some(result) = rx.recv().await {
                                if output
                                    .send(Message::ProfileView(
                                        ProfileViewMessage::ProgressResultReceived(Box::new(
                                            result,
                                        )),
                                    ))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            let _ = output
                                .send(Message::ProfileView(ProfileViewMessage::ProgressScanDone(
                                    generation,
                                )))
                                .await;
                        },
                    )
                },
            ),
            _ => Subscription::none(),
        };

    let screen_sub: Subscription<Message> = match &app.screen {
        Screen::ProfileView(_) => Subscription::none(),
        Screen::GameView(state) => game_view::subscription(state).map(Message::GameView),
    };

    Subscription::batch([
        keyboard_sub,
        worker_reply_sub,
        animation_sub,
        settings_flush_sub,
        toast_sub,
        hit_drain_sub,
        profile_progress_sub,
        screen_sub,
    ])
}

fn theme(app: &App) -> iced::Theme {
    app.context.settings.ui.theme.into()
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
        process::exit(1);
    }

    if args.len() == 2 && args[1] == "--probe" {
        worker::run_probe();
    }
    if args.len() == 3 && args[1] == "--worker" {
        let app_id: u32 = args[2].parse().unwrap_or_else(|_| {
            tracing::error!("invalid app_id: {}", args[2]);
            process::exit(2);
        });
        worker::run(app_id);
    }
    if args.len() >= 2 && args[1].starts_with("--worker") {
        tracing::info!("usage: steamlens-app --worker <app_id>");
        process::exit(2);
    }

    let swept = steamlens_core::sweep_orphans();
    if swept > 0 {
        tracing::info!("swept {swept} orphan shm region(s) at startup");
    }

    let loaded = settings::load_settings();
    let window_w = loaded.ui.window_width;
    let window_h = loaded.ui.window_height;

    const WINDOW_ICON_BYTES: &[u8] = include_bytes!("../../../assets/icon-256.png");
    let window_icon = iced::window::icon::from_file_data(WINDOW_ICON_BYTES, None).ok();

    iced::application(
        move || boot::boot_with_settings(loaded.clone()),
        update,
        view,
    )
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
    use crate::app_context::AnimationState;
    use crate::app_context::{
        CapsuleStore, ConnectivityState, GameCacheMemory, SteamSnapshot, UserState, WorkerState,
    };
    use crate::cache::{CachedLibrary, CachedLibraryEntry, CachedProfile, ClassifyResult};
    use crate::messaging::MessagingCenter;
    use crate::settings::Settings;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use steamlens_core::{STEAM_ID_64_INDIVIDUAL_MIN, UserProfile};

    impl Default for App {
        fn default() -> Self {
            Self {
                context: AppContext {
                    worker: WorkerState { current: None },
                    settings: Settings::default(),
                    settings_dirty_since: None,
                    messaging: MessagingCenter::new(),
                    game_cache: GameCacheMemory::default(),
                    user: UserState {
                        steam_root: PathBuf::from("/tmp"),
                        ..UserState::default()
                    },
                    connectivity: ConnectivityState {
                        steam_running: Some(true),
                        user_logged_in: Some(true),
                    },
                    no_ach_cache: cache::NoAchievementsCache::new(),
                    steam: SteamSnapshot::default(),
                    capsules: CapsuleStore::default(),
                    animation: AnimationState::new(),
                },
                screen: Screen::ProfileView(Box::new(ProfileViewState::new())),
                preserved_profile_state: None,
                boot: BootStage {
                    splash_min_elapsed: true,
                    library_cache_resolved: true,
                    cache_classified: true,
                    probe_done: true,
                    probe_classified: true,
                },
                modals: Modals::default(),
            }
        }
    }

    #[tokio::test]
    async fn boot_starts_in_profile_view() {
        let (app, _task) = boot::boot_with_settings(Settings::default());
        assert!(matches!(app.screen, Screen::ProfileView(_)));
        assert!(
            app.context.worker.current.is_some(),
            "worker must be spawned immediately"
        );
    }

    fn make_app_probing() -> App {
        App {
            screen: Screen::ProfileView(Box::new(ProfileViewState::new())),
            preserved_profile_state: None,
            boot: BootStage::default(),
            modals: Modals::default(),
            context: AppContext {
                connectivity: ConnectivityState::default(),
                user: UserState::default(),
                ..App::default().context
            },
        }
    }

    #[test]
    fn probe_result_ok_overrides_profile_and_marks_steam_running() {
        let mut app = make_app_probing();
        let probed = ProbedProfile {
            steam_id: 76561198000000042,
            nickname: "TestUser".to_owned(),
            avatar_image: Some(vec![0x89, 0x50, 0x4E, 0x47]),
            game_summaries: vec![],
            steam_level: Some(17),
            steam_root: None,
        };
        let _t = update(&mut app, Message::ProbeResult(Ok(Box::new(probed))));

        assert_eq!(app.context.connectivity.steam_running, Some(true));
        assert_eq!(app.context.connectivity.user_logged_in, Some(true));
        assert!(app.boot.probe_done);
        let profile = app
            .context
            .user
            .profile
            .as_ref()
            .expect("profile must be set");
        assert_eq!(profile.nickname, "TestUser");
        assert_eq!(profile.steam_id, 76561198000000042);
        assert_eq!(
            app.context.user.account_id,
            (76561198000000042u64 - STEAM_ID_64_INDIVIDUAL_MIN) as u32
        );
        assert!(app.context.user.avatar_handle.is_some());
    }

    #[test]
    fn probe_result_err_preserves_existing_profile() {
        let mut app = make_app_probing();
        let prior = UserProfile {
            steam_id: 1,
            nickname: "DiskFallback".to_owned(),
            avatar_png_bytes: None,
        };
        app.context.user.profile = Some(prior);

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );

        assert_eq!(app.context.connectivity.steam_running, Some(false));
        assert!(app.boot.probe_done);
        let profile = app
            .context
            .user
            .profile
            .as_ref()
            .expect("profile must be preserved");
        assert_eq!(profile.nickname, "DiskFallback");
        assert_eq!(profile.steam_id, 1);
    }

    #[test]
    fn probe_result_err_with_no_prior_profile_keeps_none() {
        let mut app = make_app_probing();
        assert!(
            app.context.user.profile.is_none(),
            "precondition: no prior profile"
        );

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::Other("timeout".to_owned()))),
        );

        assert_eq!(app.context.connectivity.steam_running, None);
        assert!(app.boot.probe_done);
        assert!(
            app.context.user.profile.is_none(),
            "no profile should remain None on probe error without disk fallback"
        );
    }

    fn splash_visible(app: &App) -> bool {
        !app.boot.is_ready()
    }

    #[test]
    fn splash_stays_until_all_four_signals_arrive() {
        let mut app = make_app_probing();
        assert!(splash_visible(&app), "all four pending → splash visible");

        app.boot.splash_min_elapsed = true;
        assert!(splash_visible(&app), "only min-elapsed → splash visible");

        app.boot.library_cache_resolved = true;
        assert!(
            splash_visible(&app),
            "min+library but no classify+probe → splash visible"
        );

        app.boot.cache_classified = true;
        assert!(
            splash_visible(&app),
            "min+library+classify but no probe → splash visible"
        );

        app.boot.probe_done = true;
        assert!(!splash_visible(&app), "all four done → splash hidden");
    }

    #[test]
    fn splash_hidden_only_after_library_cache_classify_and_probe_resolve() {
        let mut app = make_app_probing();
        app.boot.splash_min_elapsed = true;
        app.boot.library_cache_resolved = true;
        app.boot.cache_classified = true;
        assert!(splash_visible(&app), "missing probe → still visible");

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );
        assert!(
            app.boot.probe_done,
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
        app.boot.splash_min_elapsed = true;

        let _t = update(
            &mut app,
            Message::ProbeResult(Err(ProbeFailure::SteamNotRunning)),
        );

        assert!(
            app.boot.probe_done,
            "probe_done must be set on ProbeResult(Err)"
        );
        assert!(
            !app.boot.library_cache_resolved,
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
        let _t2 = update(
            &mut app,
            Message::Cache(cache::CacheEvent::LibraryLoaded(Some(cached))),
        );

        assert!(
            app.boot.library_cache_resolved,
            "library_cache_resolved must be set after LibraryCacheLoaded"
        );
        assert!(
            splash_visible(&app),
            "splash must remain visible until cache_classified resolves"
        );

        let _t3 = update(
            &mut app,
            Message::Cache(cache::CacheEvent::Classified(
                crate::cache::ClassifyResult::default(),
            )),
        );
        assert!(
            app.boot.cache_classified,
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
            schema_version: 4,
            steam_id: 76561198000000042,
            nickname: "FromCache".to_owned(),
            avatar_png_bytes: None,
            cached_at: 0,
            steam_root: None,
            steam_level: None,
        };
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::ProfileLoaded(Some(cached))),
        );
        let profile = app
            .context
            .user
            .profile
            .as_ref()
            .expect("profile must be set");
        assert_eq!(profile.nickname, "FromCache");
        assert_eq!(
            app.context.user.account_id,
            (76561198000000042u64 - STEAM_ID_64_INDIVIDUAL_MIN) as u32
        );
    }

    #[test]
    fn profile_cache_loaded_skipped_when_probe_succeeded_first() {
        let mut app = make_app_probing();
        app.context.connectivity.steam_running = Some(true);
        app.context.user.profile = Some(UserProfile {
            steam_id: 1,
            nickname: "LiveFromProbe".to_owned(),
            avatar_png_bytes: None,
        });
        let cached = CachedProfile {
            schema_version: 4,
            steam_id: 999,
            nickname: "ShouldNotWin".to_owned(),
            avatar_png_bytes: None,
            steam_root: None,
            cached_at: 0,
            steam_level: None,
        };
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::ProfileLoaded(Some(cached))),
        );
        let profile = app.context.user.profile.as_ref().unwrap();
        assert_eq!(
            profile.nickname, "LiveFromProbe",
            "probe-Ok profile must not be overwritten by cache"
        );
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
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::LibraryLoaded(Some(cached))),
        );
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
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::LibraryLoaded(Some(cached))),
        );
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
    fn persistent_cache_written_logs_error_but_returns_no_task() {
        let mut app = make_app_probing();
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::PersistentWritten(
                "profile",
                Err("disk full".to_owned()),
            )),
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
        use steamlens_core::AchievementSummary;

        let mut percentages = HashMap::new();
        for (id, _, pct) in &achievements {
            if let Some(pct) = pct {
                percentages.insert(id.clone(), *pct);
            }
        }

        let achievement_data: Vec<AchievementSummary> = achievements
            .into_iter()
            .map(|(id, achieved, _)| AchievementSummary {
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
        let entry = game_cache_builder::build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            0,
            &HashMap::new(),
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
        let entry = game_cache_builder::build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            0,
            &HashMap::new(),
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
        let entry = game_cache_builder::build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            0,
            &HashMap::new(),
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
        let entry = game_cache_builder::build_cache_entry_from_scan(
            &scanned,
            game.app_id,
            game.name.as_deref(),
            0,
            &HashMap::new(),
        );
        assert!(
            entry.tier_breakdown.is_empty(),
            "no global_percent → no tier classification"
        );
    }

    #[test]
    fn progress_result_failure_records_failed_app_id() {
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
        }

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::ProgressResultReceived(Box::new(
                ProgressResult {
                    app_id: 105600,
                    data: None,
                    error: None,
                },
            ))),
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

    #[tokio::test]
    async fn retry_failed_scans_clears_set_and_spawns_scanner() {
        let mut app = make_app_probing();
        if let Screen::ProfileView(pv) = &mut app.screen {
            pv.failed_app_ids.insert(105600);
            pv.failed_app_ids.insert(570);
        }

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::FailedScansRetryRequested),
        );

        if let Screen::ProfileView(pv) = &app.screen {
            assert!(
                pv.failed_app_ids.is_empty(),
                "failed set must be cleared after retry"
            );
            assert!(pv.progress_scanner.is_some(), "new scanner must be spawned");
            assert!(
                pv.progress_rx
                    .lock()
                    .expect("progress_rx poisoned")
                    .is_some(),
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
            Message::ProfileView(ProfileViewMessage::FailedScansRetryRequested),
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
    fn connected_reply_updates_game_name() {
        use game_view::handle_steam_reply;
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(105600);
        let _task = handle_steam_reply(
            &mut state,
            SteamReply::Connected {
                app_name: Some("Terraria".to_owned()),
            },
            &mut App::default().context,
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
            SteamReply::Connected { app_name: None },
            &mut App::default().context,
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
            !state.achievements[0].is_revealed,
            "precondition: revealed must be false"
        );

        let mut app = App::default();
        let _task = update(
            &mut state,
            GameViewMessage::RevealHidden("ACH_SECRET".to_owned()),
            &mut app.context,
        );

        assert!(
            state.achievements[0].is_revealed,
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
            is_revealed: bool,
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
            r.is_revealed = is_revealed;
            r.has_appeared = true;
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
            &HashSet::new(),
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
        zebra.has_appeared = true;

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
        ant.has_appeared = true;

        let achievements = vec![zebra, ant];
        let ids = visible_achievement_ids(
            &achievements,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &HashSet::new(),
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
            r.has_appeared = true;
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
            &HashSet::new(),
            false,
        );

        assert_eq!(ids[0], "A", "apple first (case-insensitive)");
        assert_eq!(ids[1], "B", "Banana second");
        assert_eq!(ids[2], "C", "cherry third");
    }

    #[test]
    fn global_percentages_reply_populates_rarity() {
        use game_view::handle_steam_reply;
        use game_view::types::{AchievementData, AchievementRow};
        use steam_worker::SteamReply;

        let mut state = GameViewState::new(0);

        let make_achievement_row = |id: &str| {
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

        state.achievements = vec![
            make_achievement_row("ACH_RARE"),
            make_achievement_row("ACH_COMMON"),
        ];

        let mut map = HashMap::new();
        map.insert("ACH_RARE".to_owned(), 4.0f32);
        map.insert("ACH_COMMON".to_owned(), 55.0f32);

        let _task = handle_steam_reply(
            &mut state,
            SteamReply::GlobalPercentagesReady(map),
            &mut App::default().context,
        );

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
        row.is_revealed = true;
        state.achievements.push(row);

        let _task = handle_steam_reply(
            &mut state,
            SteamReply::AchievementsFull {
                achievements: vec![data],
                stats: vec![],
            },
            &mut App::default().context,
        );

        assert!(
            state.achievements[0].is_revealed,
            "revealed state must survive AchievementsFull refresh"
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

    #[test]
    fn progress_result_success_path_inserts_cached_entry() {
        use steamlens_core::AchievementSummary;

        let app_id: u32 = 105600;
        let mut profile_view_state = ProfileViewState::new();
        profile_view_state.games.push(make_game_entry(app_id, 7));

        let mut app = App {
            screen: Screen::ProfileView(Box::new(profile_view_state)),
            ..App::default()
        };

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::ProgressResultReceived(Box::new(
                progress_scan::ProgressResult {
                    app_id,
                    data: Some(progress_scan::ScannedGameData {
                        app_name: Some("Terraria".to_owned()),
                        achievements: vec![AchievementSummary {
                            id: "ACH_KILL_BOSS".to_owned(),
                            is_achieved: true,
                        }],
                        stats: vec![],
                        global_percentages: HashMap::new(),
                        genre: None,
                    }),
                    error: None,
                },
            ))),
        );

        assert!(
            app.context.game_cache.entries.contains_key(&app_id),
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

    #[test]
    fn progress_result_empty_achievements_marks_no_ach_cache() {
        let app_id: u32 = 99999;
        let change_number: u32 = 42;
        let mut profile_view_state = ProfileViewState::new();
        profile_view_state
            .games
            .push(make_game_entry(app_id, change_number));

        let mut app = App {
            screen: Screen::ProfileView(Box::new(profile_view_state)),
            ..App::default()
        };

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::ProgressResultReceived(Box::new(
                progress_scan::ProgressResult {
                    app_id,
                    data: Some(progress_scan::ScannedGameData {
                        app_name: None,
                        achievements: vec![],
                        stats: vec![],
                        global_percentages: HashMap::new(),
                        genre: None,
                    }),
                    error: None,
                },
            ))),
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
            !app.context.game_cache.entries.contains_key(&app_id),
            "empty-achievements scan must drop any cached_entries for the app"
        );
    }

    #[test]
    fn progress_result_failed_scan_records_failed_app_id() {
        let app_id: u32 = 12345;
        let mut profile_view_state = ProfileViewState::new();
        profile_view_state.games.push(make_game_entry(app_id, 0));

        let mut app = App {
            screen: Screen::ProfileView(Box::new(profile_view_state)),
            ..App::default()
        };

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::ProgressResultReceived(Box::new(
                progress_scan::ProgressResult {
                    app_id,
                    data: None,
                    error: None,
                },
            ))),
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
        let profile_view_state = ProfileViewState::new();
        let mut app = App {
            screen: Screen::ProfileView(Box::new(profile_view_state)),
            ..App::default()
        };
        app.context.game_cache.entries.insert(
            app_id,
            cache::GameCacheEntry {
                schema_version: cache::CURRENT_SCHEMA_VERSION,
                app_id,
                name: "Team Fortress 2".to_owned(),
                cached_change_number: 0,
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
                playtime_minutes: None,
            },
        );

        let _t = update(
            &mut app,
            Message::ProfileView(ProfileViewMessage::GameOpenRequested(app_id)),
        );
        assert!(
            matches!(app.screen, Screen::GameView(_)),
            "GameOpenRequested must switch to GameView screen"
        );
        assert!(
            app.context.worker.current.is_some(),
            "GameOpenRequested must respawn worker for the new app"
        );
        assert!(
            app.context.game_cache.entries.contains_key(&app_id),
            "cached entry must survive GameOpenRequested"
        );

        if let Screen::GameView(_) = &app.screen {
            assert!(
                app.preserved_profile_state.is_some(),
                "preserved_profile_state must be stored at App level when navigating to GameView"
            );
        } else {
            panic!("expected GameView screen");
        }

        let _t = update(
            &mut app,
            Message::GameView(GameViewMessage::GoBackRequested),
        );
        assert!(
            matches!(app.screen, Screen::ProfileView(_)),
            "GoBackRequested from GameView must restore ProfileView screen"
        );
        assert!(
            app.context.game_cache.entries.contains_key(&app_id),
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
            splash::has_active_skeletons(&app),
            "WaitingStats must activate skeleton subscription"
        );
    }

    #[test]
    fn has_active_skeletons_true_for_game_view_connecting() {
        let app = make_app_with_game_view_phase(game_view::GameViewPhase::Connecting);
        assert!(
            splash::has_active_skeletons(&app),
            "Connecting must activate skeleton subscription"
        );
    }

    #[test]
    fn has_active_skeletons_false_for_game_view_ready_with_populated_achievements() {
        let mut app = make_app_with_game_view_phase(game_view::GameViewPhase::Ready);
        if let Screen::GameView(state) = &mut app.screen {
            use crate::game_view::types::{AchievementData, AchievementRow};
            let data = AchievementData {
                id: "ACH".to_owned(),
                display_name: "x".to_owned(),
                description: String::new(),
                is_hidden: false,
                is_achieved: true,
                unlock_time: None,
                permission: 0,
                icon: Some(steamlens_core::AchievementIcon {
                    width: 1,
                    height: 1,
                    rgba: vec![0; 4],
                }),
            };
            let mut row = AchievementRow::from(data);
            row.rarity_percent = Some(50.0);
            state.achievements = vec![row];
        }
        assert!(
            !splash::has_active_skeletons(&app),
            "Ready phase with hydrated achievements must NOT activate skeleton subscription"
        );
    }

    fn make_app_with_n_games(n: u32) -> App {
        let mut profile_view_state = ProfileViewState::new();
        for i in 1..=n {
            profile_view_state.games.push(make_game_entry(i, 0));
        }
        App {
            screen: Screen::ProfileView(Box::new(profile_view_state)),
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
                    cached_change_number: 0,
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
                    playtime_minutes: None,
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
        let _t = update(
            &mut app,
            Message::Cache(cache::CacheEvent::Classified(result)),
        );
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
