use iced::widget::{button, center, column, text};
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
}

fn boot() -> (App, Task<Message>) {
    let app = App {
        screen: Screen::Loading,
    };
    let task = Task::perform(connect_steam(), Message::SteamConnectAttempted);
    (app, task)
}

async fn connect_steam() -> Result<u64, String> {
    tokio::task::spawn_blocking(|| {
        steamlens_core::connect()
            .map(|c| c.steam_id())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r)
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SteamConnectAttempted(Ok(steam_id)) => {
            app.screen = Screen::Connected { steam_id };
            Task::none()
        }
        Message::SteamConnectAttempted(Err(reason)) => {
            app.screen = Screen::SteamNotRunning { reason };
            Task::none()
        }
        Message::Retry => {
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
            text(reason.as_str()).size(12),
            button(text("Retry")).on_press(Message::Retry),
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
