use std::path::PathBuf;

use iced::{
    Element, Task,
    widget::{button, column, row, text},
};

#[derive(Debug)]
struct AppState {
    current_dir: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            current_dir: std::env::current_dir().unwrap(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Exit,
}

fn update(_state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::Exit => iced::exit(),
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    column![
        row![
            text(state.current_dir.to_str().unwrap_or("unknown dir")).size(24),
            button(text("Exit").size(24)).on_press(Message::Exit),
        ]
        .spacing(8)
    ]
    .into()
}

fn theme(_state: &AppState) -> iced::Theme {
    iced::Theme::Dracula
}

fn main() -> iced::Result {
    iced::application(AppState::default, update, view)
        .title("SteamLens")
        .theme(theme)
        .run()
}
