mod manager;
mod steam_worker;

use std::sync::mpsc;

use iced::keyboard;
use iced::widget::{button, center, column, container, row, text, text_input};
use iced::{Element, Length, Padding, Subscription, Task};

use manager::{ManagerMessage, ManagerState};
use steam_worker::{SteamReply, SteamRequest, SteamWorker};

#[derive(Debug)]
enum Screen {
    Splash,
    Picker { app_id_input: String },
    SteamNotRunning { reason: String },
    Manager(Box<ManagerState>),
}

#[derive(Debug, Clone)]
enum Message {
    SplashDone,
    Exit,
    GoBack,
    AppIdInputChanged(String),
    OpenManager,
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
}

fn boot() -> (App, Task<Message>) {
    let app = App {
        screen: Screen::Splash,
        worker: None,
        worker_rx: None,
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

        let Screen::Manager(state) = &mut app.screen else {
            continue;
        };

        match &reply {
            SteamReply::Connected { .. } => {
                if let Some(w) = &app.worker {
                    w.send(SteamRequest::RequestUserStats);
                }
            }
            SteamReply::ResetDone => {
                if let Some(w) = &app.worker {
                    w.send(SteamRequest::RequestUserStats);
                }
            }
            _ => {}
        }

        let _task = manager::handle_steam_reply(state, reply);
    }

    Task::none()
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SplashDone => {
            app.screen = Screen::Picker {
                app_id_input: String::new(),
            };
            Task::none()
        }
        Message::Exit => iced::exit(),
        Message::GoBack => {
            if let Screen::Manager(_) = &app.screen {
                if let Some(w) = &app.worker {
                    w.send(SteamRequest::Disconnect);
                }
                app.worker = None;
                app.worker_rx = None;
            }
            app.screen = Screen::Picker {
                app_id_input: String::new(),
            };
            Task::none()
        }
        Message::AppIdInputChanged(s) => {
            if let Screen::Picker { app_id_input } = &mut app.screen {
                *app_id_input = s.chars().filter(|c| c.is_ascii_digit()).collect();
            }
            Task::none()
        }
        Message::OpenManager => {
            let app_id: u32 = if let Screen::Picker { app_id_input } = &app.screen {
                app_id_input.parse().unwrap_or(0)
            } else {
                0
            };

            if app_id == 0 {
                return Task::none();
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

        Screen::Picker { app_id_input } => picker_view(app_id_input),

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

fn picker_view(app_id_input: &str) -> Element<'_, Message> {
    let header = text("SteamLens").size(32);
    let subtitle = text("Enter a Steam App ID to inspect achievements and stats.").size(14);

    let input_row = row![
        text_input("App ID (e.g. 105600)", app_id_input)
            .on_input(Message::AppIdInputChanged)
            .on_submit(Message::OpenManager)
            .padding(10)
            .size(14)
            .width(Length::Fixed(220.0)),
        button(text("Open Manager").size(14))
            .on_press(Message::OpenManager)
            .padding(Padding::from([10u16, 18])),
    ]
    .spacing(8);

    let content = column![header, subtitle, input_row].spacing(16).padding(24);

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

    Subscription::batch([keyboard_sub, poll_sub, manager_sub])
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

    fn make_app_picker() -> App {
        App {
            screen: Screen::Picker {
                app_id_input: String::new(),
            },
            worker: None,
            worker_rx: None,
        }
    }

    fn make_app_not_running(reason: &str) -> App {
        App {
            screen: Screen::SteamNotRunning {
                reason: reason.to_owned(),
            },
            worker: None,
            worker_rx: None,
        }
    }

    fn screen_name(app: &App) -> &'static str {
        match app.screen {
            Screen::Splash => "Splash",
            Screen::Picker { .. } => "Picker",
            Screen::SteamNotRunning { .. } => "SteamNotRunning",
            Screen::Manager(_) => "Manager",
        }
    }

    fn make_app_splash() -> App {
        App {
            screen: Screen::Splash,
            worker: None,
            worker_rx: None,
        }
    }

    #[test]
    fn boot_starts_in_splash() {
        let (app, _task) = boot();
        assert!(matches!(app.screen, Screen::Splash));
        assert!(app.worker.is_none());
    }

    #[test]
    fn splash_done_transitions_to_picker() {
        let mut app = make_app_splash();
        let _task = update(&mut app, Message::SplashDone);
        assert!(
            matches!(app.screen, Screen::Picker { .. }),
            "expected Picker after SplashDone, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn app_id_input_filters_non_digits() {
        let mut app = make_app_picker();
        let _task = update(&mut app, Message::AppIdInputChanged("105abc600".to_owned()));
        if let Screen::Picker { app_id_input } = &app.screen {
            assert_eq!(app_id_input, "105600");
        } else {
            panic!("expected Picker");
        }
    }

    #[test]
    fn open_manager_with_zero_app_id_is_no_op() {
        let mut app = make_app_picker();
        let _task = update(&mut app, Message::OpenManager);
        assert!(
            matches!(app.screen, Screen::Picker { .. }),
            "expected Picker after OpenManager with empty input, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn go_back_from_not_running_returns_to_picker() {
        let mut app = make_app_not_running("pipe closed");
        let _task = update(&mut app, Message::GoBack);
        assert!(
            matches!(app.screen, Screen::Picker { .. }),
            "expected Picker after GoBack, got {}",
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
}
