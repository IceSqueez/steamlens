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
    Loading,
    SteamNotRunning { reason: String },
    Connected { steam_id: u64, app_id_input: String },
    Manager(Box<ManagerState>),
}

#[derive(Debug, Clone)]
enum Message {
    SteamConnectAttempted(Result<u64, String>),
    Retry,
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
    connecting: bool,
    worker: Option<SteamWorker>,
    worker_rx: Option<mpsc::Receiver<SteamReply>>,
}

fn boot() -> (App, Task<Message>) {
    let app = App {
        screen: Screen::Loading,
        connecting: true,
        worker: None,
        worker_rx: None,
    };
    let task = Task::perform(connect_steam_initial(), Message::SteamConnectAttempted);
    (app, task)
}

async fn connect_steam_initial() -> Result<u64, String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(|| {
            steamlens_core::connect(0)
                .map(|c| c.steam_id())
                .map_err(|e| e.to_string())
        }),
    )
    .await
    .map_err(|_| "Connection timed out after 10s — is Steam responsive?".to_string())
    .and_then(|join_result| join_result.map_err(|e| e.to_string()))
    .and_then(|r| r)
}

fn drain_worker_replies(app: &mut App) -> Task<Message> {
    let Some(rx) = &app.worker_rx else {
        return Task::none();
    };

    let replies: Vec<SteamReply> = rx.try_iter().collect();

    for reply in replies {
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
            SteamReply::Callback(cb) => {
                use steamlens_core::SteamCallback;
                if let SteamCallback::UserStatsReceived { result, .. } = cb
                    && result.is_ok()
                    && let Some(w) = &app.worker
                {
                    w.send(SteamRequest::LoadAchievementsAndStats);
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
        Message::SteamConnectAttempted(Ok(steam_id)) => {
            app.connecting = false;
            app.screen = Screen::Connected {
                steam_id,
                app_id_input: String::new(),
            };
            Task::none()
        }
        Message::SteamConnectAttempted(Err(reason)) => {
            app.connecting = false;
            app.screen = Screen::SteamNotRunning { reason };
            Task::none()
        }
        Message::Retry => {
            if app.connecting {
                return Task::none();
            }
            app.connecting = true;
            app.screen = Screen::Loading;
            Task::perform(connect_steam_initial(), Message::SteamConnectAttempted)
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
            app.screen = Screen::Connected {
                steam_id: 0,
                app_id_input: String::new(),
            };
            Task::perform(connect_steam_initial(), Message::SteamConnectAttempted)
        }
        Message::AppIdInputChanged(s) => {
            if let Screen::Connected { app_id_input, .. } = &mut app.screen {
                *app_id_input = s.chars().filter(|c| c.is_ascii_digit()).collect();
            }
            Task::none()
        }
        Message::OpenManager => {
            let app_id: u32 = if let Screen::Connected { app_id_input, .. } = &app.screen {
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
        Screen::Loading => {
            let content: Element<'_, Message> = column![
                text("Connecting to Steam...").size(20),
                text("Please wait...").size(14),
            ]
            .spacing(16)
            .into();
            center(content).into()
        }

        Screen::SteamNotRunning { reason } => {
            let content: Element<'_, Message> = column![
                text("Steam is not running").size(28),
                text("Start the Steam client and try again.").size(16),
                text(reason.as_str()).size(14),
                row![
                    button(text("Retry")).on_press(Message::Retry),
                    button(text("Exit")).on_press(Message::Exit),
                ]
                .spacing(8),
            ]
            .spacing(16)
            .into();
            center(content).into()
        }

        Screen::Connected {
            steam_id,
            app_id_input,
        } => connected_view(*steam_id, app_id_input),

        Screen::Manager(state) => manager::view(state),
    }
}

fn connected_view(steam_id: u64, app_id_input: &str) -> Element<'_, Message> {
    let header = text("Connected to Steam").size(24);
    let id_text = text(format!("Steam ID: {steam_id}")).size(14);

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

    let hint = text("Enter a Steam App ID to open the achievement manager.").size(13);

    let content = column![header, id_text, input_row, hint]
        .spacing(16)
        .padding(24);

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

    fn make_app_loading() -> App {
        App {
            screen: Screen::Loading,
            connecting: true,
            worker: None,
            worker_rx: None,
        }
    }

    fn make_app_not_running(reason: &str) -> App {
        App {
            screen: Screen::SteamNotRunning {
                reason: reason.to_owned(),
            },
            connecting: false,
            worker: None,
            worker_rx: None,
        }
    }

    fn make_app_connected(steam_id: u64) -> App {
        App {
            screen: Screen::Connected {
                steam_id,
                app_id_input: String::new(),
            },
            connecting: false,
            worker: None,
            worker_rx: None,
        }
    }

    fn screen_name(app: &App) -> &'static str {
        match app.screen {
            Screen::Loading => "Loading",
            Screen::SteamNotRunning { .. } => "SteamNotRunning",
            Screen::Connected { .. } => "Connected",
            Screen::Manager(_) => "Manager",
        }
    }

    #[test]
    fn connect_ok_transitions_to_connected() {
        let mut app = make_app_loading();
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Ok(76561198000000001)),
        );
        match app.screen {
            Screen::Connected { steam_id, .. } => assert_eq!(steam_id, 76561198000000001),
            _ => panic!("expected Connected, got {}", screen_name(&app)),
        }
    }

    #[test]
    fn connect_err_transitions_to_steam_not_running() {
        let mut app = make_app_loading();
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Err("Steam not running".to_owned())),
        );
        match &app.screen {
            Screen::SteamNotRunning { reason } => {
                assert_eq!(reason, "Steam not running");
            }
            _ => panic!("expected SteamNotRunning, got {}", screen_name(&app)),
        }
    }

    #[test]
    fn retry_from_not_running_transitions_to_loading() {
        let mut app = make_app_not_running("pipe closed");
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn retry_from_connected_transitions_to_loading() {
        let mut app = make_app_connected(76561198000000001);
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry from Connected, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn retry_from_loading_is_no_op_while_connecting() {
        let mut app = make_app_loading();
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry from Loading, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn connect_ok_stores_correct_steam_id() {
        let mut app = make_app_loading();
        let _task = update(&mut app, Message::SteamConnectAttempted(Ok(0)));
        match app.screen {
            Screen::Connected { steam_id, .. } => {
                assert_eq!(steam_id, 0, "steam_id 0 must be stored as-is")
            }
            _ => panic!("expected Connected"),
        }
    }

    #[test]
    fn connect_err_stores_reason_verbatim() {
        let mut app = make_app_loading();
        let reason = "Could not locate steamclient.so.";
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Err(reason.to_owned())),
        );
        match &app.screen {
            Screen::SteamNotRunning { reason: stored } => assert_eq!(stored, reason),
            _ => panic!("expected SteamNotRunning"),
        }
    }

    #[test]
    fn retry_while_connecting_does_not_change_state() {
        let mut app = make_app_loading();
        assert!(app.connecting, "precondition: connecting must be true");
        let screen_before = screen_name(&app);
        let _task = update(&mut app, Message::Retry);
        assert_eq!(
            screen_name(&app),
            screen_before,
            "screen must not change when Retry fires while connecting"
        );
        assert!(
            app.connecting,
            "connecting flag must remain true after no-op Retry"
        );
    }

    #[test]
    fn successful_connect_clears_connecting_flag() {
        let mut app = make_app_loading();
        assert!(app.connecting);
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Ok(76561198000000001)),
        );
        assert!(!app.connecting);
    }

    #[test]
    fn failed_connect_clears_connecting_flag() {
        let mut app = make_app_loading();
        assert!(app.connecting);
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Err("Steam not running".to_owned())),
        );
        assert!(!app.connecting);
    }

    #[test]
    fn app_id_input_filters_non_digits() {
        let mut app = make_app_connected(1);
        let _task = update(&mut app, Message::AppIdInputChanged("105abc600".to_owned()));
        if let Screen::Connected { app_id_input, .. } = &app.screen {
            assert_eq!(app_id_input, "105600");
        } else {
            panic!("expected Connected");
        }
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
}
