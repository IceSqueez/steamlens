mod capsule_cache;
mod library;
mod manager;
mod steam_worker;

use std::sync::mpsc;

use iced::keyboard;
use iced::widget::{button, center, column, container, row, text};
use iced::{Element, Length, Subscription, Task};

use library::types::{LibraryMessage, LibraryState};
use manager::{ManagerMessage, ManagerState};
use steam_worker::{SteamReply, SteamRequest, SteamWorker};

#[derive(Debug)]
enum Screen {
    Splash,
    Library(Box<LibraryState>),
    SteamNotRunning { reason: String },
    Manager(Box<ManagerState>),
}

#[derive(Debug, Clone)]
enum Message {
    SplashDone,
    Exit,
    GoBack,
    Library(LibraryMessage),
    OpenManager(u32),
    Manager(ManagerMessage),
    PollWorker,
    KeyboardEvent(keyboard::Event),
}

impl std::fmt::Debug for ManagerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagerState")
            .field("app_id", &self.app_id)
            .field("phase", &self.phase)
            .finish()
    }
}

struct App {
    screen: Screen,
    worker: Option<SteamWorker>,
    worker_rx: Option<mpsc::Receiver<SteamReply>>,
    library_state: Option<Box<LibraryState>>,
}

fn boot() -> (App, Task<Message>) {
    let app = App {
        screen: Screen::Splash,
        worker: None,
        worker_rx: None,
        library_state: None,
    };
    let splash_task = Task::perform(
        async {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        },
        |()| Message::SplashDone,
    );
    (app, splash_task)
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

        match &reply {
            SteamReply::LibraryScan(_) | SteamReply::LibraryScanFailed(_) => {
                if let Screen::Library(state) = &mut app.screen {
                    let t = library::handle_steam_reply(state, reply);
                    tasks.push(t);
                }
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

        let Screen::Manager(state) = &mut app.screen else {
            continue;
        };

        let t = manager::handle_steam_reply(state, reply);
        tasks.push(t);
    }

    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SplashDone => {
            let lib_state = LibraryState::new();
            let (worker, rx) = SteamWorker::spawn();
            library::trigger_scan(&worker);
            app.worker = Some(worker);
            app.worker_rx = Some(rx);
            app.screen = Screen::Library(Box::new(lib_state));
            Task::none()
        }

        Message::Exit => iced::exit(),

        Message::GoBack => {
            let returning_from_manager_or_error =
                matches!(&app.screen, Screen::Manager(_) | Screen::SteamNotRunning { .. });
            if returning_from_manager_or_error {
                if let Some(w) = &app.worker {
                    w.send(SteamRequest::Disconnect);
                }
                app.worker = None;
                app.worker_rx = None;

                if let Some(mut stored) = app.library_state.take() {
                    stored.has_opened_a_game = true;
                    app.screen = Screen::Library(stored);
                } else {
                    let lib_state = LibraryState::new();
                    let (worker, rx) = SteamWorker::spawn();
                    library::trigger_scan(&worker);
                    app.worker = Some(worker);
                    app.worker_rx = Some(rx);
                    app.screen = Screen::Library(Box::new(lib_state));
                }
            }
            Task::none()
        }

        Message::Library(lib_msg) => {
            match &lib_msg {
                LibraryMessage::GameSelected(app_id) => {
                    let app_id = *app_id;
                    if app_id == 0 {
                        return Task::none();
                    }
                    if let Screen::Library(lib_state) = &mut app.screen {
                        lib_state.has_opened_a_game = true;
                    }
                    return update(app, Message::OpenManager(app_id));
                }
                LibraryMessage::ManualAppIdSubmitted => {
                    let app_id: u32 = if let Screen::Library(lib_state) = &app.screen {
                        lib_state.manual_app_id_input.parse().unwrap_or(0)
                    } else {
                        0
                    };
                    if app_id == 0 {
                        return Task::none();
                    }
                    if let Screen::Library(lib_state) = &mut app.screen {
                        lib_state.has_opened_a_game = true;
                    }
                    return update(app, Message::OpenManager(app_id));
                }
                LibraryMessage::RescanRequested => {
                    if let Screen::Library(lib_state) = &mut app.screen {
                        let t = library::update(lib_state, lib_msg);
                        if let Some(w) = &app.worker {
                            library::trigger_scan(w);
                        }
                        return t;
                    }
                    return Task::none();
                }
                _ => {}
            }

            if let Screen::Library(lib_state) = &mut app.screen {
                return library::update(lib_state, lib_msg);
            }
            Task::none()
        }

        Message::OpenManager(app_id) => {
            if let Some(w) = &app.worker {
                w.send(SteamRequest::Disconnect);
            }
            app.worker = None;
            app.worker_rx = None;

            if let Screen::Library(lib_state) =
                std::mem::replace(&mut app.screen, Screen::Splash)
            {
                app.library_state = Some(lib_state);
            }

            let (worker, rx) = SteamWorker::spawn();
            worker.send(SteamRequest::ConnectWithApp(app_id));

            let state = ManagerState::new(app_id);
            app.worker = Some(worker);
            app.worker_rx = Some(rx);
            app.screen = Screen::Manager(Box::new(state));

            Task::none()
        }

        Message::Manager(m) => {
            if let Screen::Manager(state) = &mut app.screen
                && let Some(worker) = &app.worker
            {
                return manager::update(state, m, worker);
            }
            Task::none()
        }

        Message::PollWorker => drain_worker_replies(app),

        Message::KeyboardEvent(event) => {
            if let keyboard::Event::KeyPressed {
                modifiers,
                key: keyboard::Key::Character(ref c),
                ..
            } = event
                && modifiers.control()
                && c.as_str() == "s"
                && let Screen::Manager(state) = &mut app.screen
                && state.dirty_count() > 0
                && !state.has_stat_errors()
                && let Some(w) = &app.worker
            {
                return manager::update(state, ManagerMessage::ApplyChanges, w);
            }
            Task::none()
        }
    }
}

fn view(app: &App) -> Element<'_, Message> {
    match &app.screen {
        Screen::Splash => splash_view(),

        Screen::Library(lib_state) => library::view(lib_state),

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

        Screen::Manager(state) => manager::view(state),
    }
}

fn splash_view() -> Element<'static, Message> {
    use iced::widget::column;

    let title = text("SteamLens")
        .size(64)
        .color(iced::Color::from_rgb(0.741, 0.576, 0.976));

    let subtitle = text("Steam achievements & stats inspector")
        .size(14)
        .color(iced::Color::from_rgb(0.384, 0.447, 0.643));

    let content = column![title, subtitle]
        .spacing(12)
        .align_x(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .into()
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

    let manager_sub = if let Screen::Manager(state) = &app.screen {
        manager::subscription(state)
    } else {
        Subscription::none()
    };

    let fade_sub = if let Screen::Library(state) = &app.screen {
        if state.has_fading_capsules() {
            iced::time::every(std::time::Duration::from_millis(33))
                .map(|_| Message::Library(LibraryMessage::FadeTick))
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    let reveal_sub = if let Screen::Library(state) = &app.screen {
        if state.has_pending_reveals() {
            iced::time::every(std::time::Duration::from_millis(150))
                .map(|_| Message::Library(LibraryMessage::RevealTick))
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    let library_spinner_sub = if let Screen::Library(state) = &app.screen {
        if state.is_streaming() {
            iced::time::every(std::time::Duration::from_millis(80))
                .map(|_| Message::Library(LibraryMessage::SpinnerTick(0.0)))
        } else {
            Subscription::none()
        }
    } else {
        Subscription::none()
    };

    Subscription::batch([
        keyboard_sub,
        poll_sub,
        manager_sub,
        fade_sub,
        reveal_sub,
        library_spinner_sub,
    ])
}

fn theme(_app: &App) -> iced::Theme {
    iced::Theme::Dracula
}

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("SteamLens")
        .theme(theme)
        .subscription(subscription)
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
            library_state: None,
        }
    }

    fn screen_name(app: &App) -> &'static str {
        match app.screen {
            Screen::Splash => "Splash",
            Screen::Library(_) => "Library",
            Screen::SteamNotRunning { .. } => "SteamNotRunning",
            Screen::Manager(_) => "Manager",
        }
    }

    fn make_app_splash() -> App {
        App {
            screen: Screen::Splash,
            worker: None,
            worker_rx: None,
            library_state: None,
        }
    }

    #[test]
    fn boot_starts_in_splash() {
        let (app, _task) = boot();
        assert!(matches!(app.screen, Screen::Splash));
        assert!(app.worker.is_none());
    }

    #[test]
    fn splash_done_transitions_to_library() {
        let mut app = make_app_splash();
        let _task = update(&mut app, Message::SplashDone);
        assert!(
            matches!(app.screen, Screen::Library(_)),
            "expected Library after SplashDone, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn go_back_from_not_running_returns_to_library() {
        let mut app = make_app_not_running("pipe closed");
        let _task = update(&mut app, Message::GoBack);
        assert!(
            matches!(app.screen, Screen::Library(_)),
            "expected Library after GoBack, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn manager_state_dirty_count_zero_on_init() {
        let state = ManagerState::new(105600);
        assert_eq!(state.dirty_count(), 0);
    }

    #[test]
    fn manager_state_no_errors_on_init() {
        let state = ManagerState::new(105600);
        assert!(!state.has_stat_errors());
    }

    fn make_manager_state_with_dirty_achievements() -> ManagerState {
        use manager::types::{AchievementData, AchievementRow};
        let mut state = ManagerState::new(105600);
        state.phase = manager::ManagerPhase::Ready;
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
        use manager::ManagerMessage;
        use steam_worker::SteamWorker;

        let mut state = make_manager_state_with_dirty_achievements();
        assert_eq!(
            state.dirty_count(),
            1,
            "precondition: one dirty achievement"
        );

        let worker = SteamWorker::new_disconnected();
        let _task = manager::update(&mut state, ManagerMessage::DiscardChanges, &worker);

        assert_eq!(
            state.dirty_count(),
            0,
            "dirty count must be zero after DiscardChanges"
        );
    }

    #[test]
    fn cancel_does_not_change_phase() {
        use manager::ManagerMessage;
        use steam_worker::SteamWorker;

        let mut state = make_manager_state_with_dirty_achievements();
        assert_eq!(state.phase, manager::ManagerPhase::Ready, "precondition");

        let worker = SteamWorker::new_disconnected();
        let _task = manager::update(&mut state, ManagerMessage::DiscardChanges, &worker);

        assert_eq!(
            state.phase,
            manager::ManagerPhase::Ready,
            "phase must not change after DiscardChanges"
        );
    }

    #[test]
    fn manager_state_app_name_starts_as_fallback() {
        let state = ManagerState::new(105600);
        assert_eq!(
            state.game_name, "App 105600",
            "initial game_name must be fallback App <id>"
        );
    }

    #[test]
    fn connected_reply_updates_game_name() {
        use manager::handle_steam_reply;
        use steam_worker::SteamReply;

        let mut state = ManagerState::new(105600);
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
        use manager::handle_steam_reply;
        use steam_worker::SteamReply;

        let mut state = ManagerState::new(105600);
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
        use manager::types::{AchievementData, AchievementRow};
        use manager::{ManagerMessage, ManagerPhase, update};
        use steam_worker::SteamWorker;

        let mut state = ManagerState::new(105600);
        state.phase = ManagerPhase::Ready;
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
            ManagerMessage::RevealHidden("ACH_SECRET".to_owned()),
            &worker,
        );

        assert!(
            state.achievements[0].revealed,
            "revealed must be true after RevealHidden"
        );
    }

    #[test]
    fn sort_orders_unlocked_then_locked_then_hidden() {
        use manager::types::{
            AchievementData, AchievementFilter, AchievementRow, visible_achievement_ids,
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

        let ids = visible_achievement_ids(&achievements, AchievementFilter::All, "");

        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], "A", "A (unlocked) first");
        assert_eq!(ids[1], "E", "E (hidden+achieved = unlocked) second");
        assert_eq!(ids[2], "B", "B (locked) before D (revealed)");
        assert_eq!(ids[3], "D", "D (revealed hidden = locked group)");
        assert_eq!(ids[4], "C", "C (hidden unrevealed) last");
    }

    #[test]
    fn dirty_unlock_does_not_change_group_until_apply() {
        use manager::types::{
            AchievementData, AchievementFilter, AchievementRow, visible_achievement_ids,
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
        let ids = visible_achievement_ids(&achievements, AchievementFilter::All, "");

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
        use manager::types::{
            AchievementData, AchievementFilter, AchievementRow, visible_achievement_ids,
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

        let ids = visible_achievement_ids(&achievements, AchievementFilter::All, "");

        assert_eq!(ids[0], "A", "apple first (case-insensitive)");
        assert_eq!(ids[1], "B", "Banana second");
        assert_eq!(ids[2], "C", "cherry third");
    }

    #[test]
    fn apply_then_reload_preserves_revealed_state() {
        use manager::handle_steam_reply;
        use manager::types::{AchievementData, AchievementRow};
        use steam_worker::SteamReply;

        let mut state = ManagerState::new(105600);
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
}
