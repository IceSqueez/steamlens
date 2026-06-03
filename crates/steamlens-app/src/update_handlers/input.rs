use iced::Task;
use iced::keyboard;

use steamlens_core::STEAMID64_INDIVIDUAL_MIN;

use crate::game_view::{self, GameViewMessage};
use crate::messaging::{self, BannerSeverity};
use crate::profile_view::{self, types::ProfileViewMessage};
use crate::{App, Message, Screen, routing};

pub(crate) fn handle_keyboard_event(app: &mut App, event: keyboard::Event) -> Task<Message> {
    if let keyboard::Event::KeyPressed {
        modifiers,
        key: keyboard::Key::Character(ref c),
        ..
    } = event
    {
        if modifiers.control()
            && c.as_str() == "s"
            && let Screen::GameView(state) = &mut app.screen
            && state.dirty_count() > 0
            && !state.has_stat_errors()
        {
            let (task, _event) =
                game_view::update(state, GameViewMessage::ApplyClicked, &mut app.context);
            return task.map(Message::GameView);
        }
        if modifiers.command() && c.as_str() == "f" {
            return Task::done(Message::FocusSearch);
        }
    }
    if let keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(keyboard::key::Named::Escape),
        ..
    } = event
    {
        if app.modals.about_open {
            return Task::done(Message::DismissAbout);
        }
        match &mut app.screen {
            Screen::ProfileView(state) if !state.search.is_empty() => {
                let (task, _event) = profile_view::update(
                    state,
                    ProfileViewMessage::SearchChanged(String::new()),
                    &mut app.context,
                );
                return task.map(Message::ProfileView);
            }
            Screen::GameView(state) if !state.search_query.is_empty() => {
                let (task, event) = game_view::update(
                    state,
                    GameViewMessage::SearchChanged(String::new()),
                    &mut app.context,
                );
                let task = task.map(Message::GameView);
                return routing::dispatch_game_event(app, task, event);
            }
            _ => {}
        }
    }
    Task::none()
}

pub(crate) fn handle_global_search_changed(app: &mut App, query: String) -> Task<Message> {
    match &mut app.screen {
        Screen::ProfileView(state) => {
            let (task, _event) = profile_view::update(
                state,
                ProfileViewMessage::SearchChanged(query),
                &mut app.context,
            );
            task.map(Message::ProfileView)
        }
        Screen::GameView(state) => {
            let (task, event) = game_view::update(
                state,
                GameViewMessage::SearchChanged(query),
                &mut app.context,
            );
            let task = task.map(Message::GameView);
            routing::dispatch_game_event(app, task, event)
        }
    }
}

pub(crate) fn handle_update_check_result(
    app: &mut App,
    result: Result<Option<crate::update_check::UpdateInfo>, String>,
) -> Task<Message> {
    match result {
        Ok(Some(info)) => {
            let body = format!(
                "A new version {} is available - click Download to get it.",
                info.latest
            );
            app.context.messaging.push_banner(
                BannerSeverity::Info,
                body,
                Some(messaging::BannerAction {
                    label: "Download",
                    message: Message::OpenUrl(info.html_url),
                }),
                false,
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("update_check: {e}");
        }
    }
    Task::none()
}

pub(crate) fn handle_local_profile_loaded(
    app: &mut App,
    profile: Option<Box<steamlens_core::UserProfile>>,
) -> Task<Message> {
    let profile = profile.map(|b| *b);
    app.context.profile_avatar_handle = profile
        .as_ref()
        .and_then(|p| p.avatar_png_bytes.as_ref())
        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));
    if let Some(p) = &profile {
        app.context.steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN) as u32;
    }
    app.context.user_profile = profile;
    Task::none()
}

pub(crate) fn handle_messaging(app: &mut App, msg: messaging::MessagingEvent) -> Task<Message> {
    match msg {
        messaging::MessagingEvent::ToastTick => app.context.messaging.tick_toasts(),
        messaging::MessagingEvent::ToastHovered(id, hovered) => {
            app.context.messaging.set_toast_hovered(id, hovered);
        }
        messaging::MessagingEvent::DismissToast(id) => {
            app.context.messaging.dismiss_toast(id);
        }
        messaging::MessagingEvent::DismissBanner(id) => {
            app.context.messaging.dismiss_banner(id);
        }
    }
    Task::none()
}
