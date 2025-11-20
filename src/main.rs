use std::path::PathBuf;

use iced::{
    Element, Task,
    widget::{button, column, row, text},
    window,
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

fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::Exit => window::get_latest().and_then(window::close),
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    column![
        row![
            text(state.current_dir.to_str().unwrap_or("unknown dir")).size(24),
            button(text("Exit").size(24)).on_press(Message::Exit),
            button(text("Up").size(24)).on_press(Message::Exit)
        ]
        .spacing(8)
    ]
    .into()
}

fn main() -> iced::Result {
    iced::application("SteamLens", update, view)
        .theme(|_x| iced::Theme::Dracula)
        .run()
}
