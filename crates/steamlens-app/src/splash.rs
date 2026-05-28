use iced::widget::{column, container, text};
use iced::{Color, Element};

use crate::game_view;
use crate::{App, Message, Screen};

pub(crate) fn splash_status_text(app: &App) -> &'static str {
    if !app.boot.splash_min_elapsed {
        "starting up\u{2026}"
    } else if !app.boot.probe_done {
        "connecting to Steam\u{2026}"
    } else if !app.boot.library_cache_resolved {
        "loading library\u{2026}"
    } else if !app.boot.cache_classified {
        "reading cache\u{2026}"
    } else {
        "almost ready\u{2026}"
    }
}

pub(crate) fn splash_view<'a>(status: &'static str) -> Element<'a, Message> {
    let title = text("SteamLens")
        .size(40)
        .color(Color::from_rgb(0.741, 0.576, 0.976));
    let subtitle = text(status)
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

pub(crate) fn has_active_skeletons(app: &App) -> bool {
    match &app.screen {
        Screen::ProfileView(pv) => pv.games.iter().any(|g| !g.is_hydrated()),
        Screen::GameView(state) => {
            if state.cache_only {
                return state.achievements.is_empty();
            }
            matches!(
                state.phase,
                game_view::GameViewPhase::Connecting | game_view::GameViewPhase::WaitingStats
            ) || state.achievements.is_empty()
                || state.achievements.iter().any(|r| {
                    if r.is_spoiler_hidden() {
                        return false;
                    }
                    r.data.icon.is_none() || r.rarity_percent.is_none()
                })
        }
    }
}
