use iced::widget::{button, center, column, row, text};
use iced::{Element, Task};

#[derive(Debug)]
enum Screen {
    Loading,
    SteamNotRunning { reason: String },
    Connected { steam_id: u64 },
}

#[derive(Debug, Clone)]
enum Message {
    SteamConnectAttempted(Result<u64, String>),
    Retry,
    Exit,
}

struct App {
    screen: Screen,
    connecting: bool,
}

fn boot() -> (App, Task<Message>) {
    let app = App {
        screen: Screen::Loading,
        connecting: true,
    };
    let task = Task::perform(connect_steam(), Message::SteamConnectAttempted);
    (app, task)
}

async fn connect_steam() -> Result<u64, String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(|| {
            steamlens_core::connect()
                .map(|c| c.steam_id())
                .map_err(|e| e.to_string())
        }),
    )
    .await
    .map_err(|_| "Connection timed out after 10s — is Steam responsive?".to_string())
    .and_then(|join_result| join_result.map_err(|e| e.to_string()))
    .and_then(|r| r)
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SteamConnectAttempted(Ok(steam_id)) => {
            app.connecting = false;
            app.screen = Screen::Connected { steam_id };
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
            Task::perform(connect_steam(), Message::SteamConnectAttempted)
        }
        Message::Exit => iced::exit(),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = match &app.screen {
        Screen::Loading => column![text("Connecting to Steam...").size(20)]
            .spacing(16)
            .into(),

        Screen::SteamNotRunning { reason } => column![
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
        .into(),

        Screen::Connected { steam_id } => column![
            text("Connected to Steam").size(28),
            text(format!("Steam ID: {steam_id}")).size(16),
            button(text("Exit")).on_press(Message::Exit),
        ]
        .spacing(16)
        .into(),
    };

    center(content).into()
}

fn theme(_app: &App) -> iced::Theme {
    iced::Theme::Dracula
}

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("SteamLens")
        .theme(theme)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_loading() -> App {
        App {
            screen: Screen::Loading,
            connecting: true,
        }
    }

    fn app_not_running(reason: &str) -> App {
        App {
            screen: Screen::SteamNotRunning {
                reason: reason.to_owned(),
            },
            connecting: false,
        }
    }

    fn app_connected(steam_id: u64) -> App {
        App {
            screen: Screen::Connected { steam_id },
            connecting: false,
        }
    }

    fn screen_name(app: &App) -> &'static str {
        match app.screen {
            Screen::Loading => "Loading",
            Screen::SteamNotRunning { .. } => "SteamNotRunning",
            Screen::Connected { .. } => "Connected",
        }
    }

    #[test]
    fn connect_ok_transitions_to_connected() {
        let mut app = app_loading();
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Ok(76561198000000001)),
        );
        match app.screen {
            Screen::Connected { steam_id } => assert_eq!(steam_id, 76561198000000001),
            _ => panic!("expected Connected, got {}", screen_name(&app)),
        }
    }

    #[test]
    fn connect_err_transitions_to_steam_not_running() {
        let mut app = app_loading();
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
        let mut app = app_not_running("pipe closed");
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn retry_from_connected_transitions_to_loading() {
        let mut app = app_connected(76561198000000001);
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry from Connected, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn retry_from_loading_transitions_to_loading() {
        let mut app = app_loading();
        let _task = update(&mut app, Message::Retry);
        assert!(
            matches!(app.screen, Screen::Loading),
            "expected Loading after Retry from Loading, got {}",
            screen_name(&app)
        );
    }

    #[test]
    fn connect_ok_stores_correct_steam_id() {
        let mut app = app_loading();
        let _task = update(&mut app, Message::SteamConnectAttempted(Ok(0)));
        match app.screen {
            Screen::Connected { steam_id } => {
                assert_eq!(steam_id, 0, "steam_id 0 must be stored as-is")
            }
            _ => panic!("expected Connected"),
        }
    }

    #[test]
    fn connect_err_stores_reason_verbatim() {
        let mut app = app_loading();
        let reason = "Could not locate steamclient.so. Searched: /home/x/.steam/steam/linux64/steamclient.so";
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
        let mut app = app_loading();
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
        let mut app = app_loading();
        assert!(app.connecting, "precondition: connecting must be true");
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Ok(76561198000000001)),
        );
        assert!(
            !app.connecting,
            "connecting must be false after successful connect"
        );
    }

    #[test]
    fn failed_connect_clears_connecting_flag() {
        let mut app = app_loading();
        assert!(app.connecting, "precondition: connecting must be true");
        let _task = update(
            &mut app,
            Message::SteamConnectAttempted(Err("Steam not running".to_owned())),
        );
        assert!(
            !app.connecting,
            "connecting must be false after failed connect"
        );
    }
}
