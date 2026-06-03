use iced::Task;

use crate::settings::{self, Settings};

pub fn write_settings(snapshot: Settings) -> Task<crate::Message> {
    Task::perform(
        async move {
            settings::write_settings(&snapshot)
                .await
                .map_err(|e| e.to_string())
        },
        crate::Message::SettingsWritten,
    )
}
