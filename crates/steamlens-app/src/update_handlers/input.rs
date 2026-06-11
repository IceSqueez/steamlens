use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use iced::Task;
use iced::keyboard;

use steamlens_core::{AppLibraryAssets, STEAM_ID_64_INDIVIDUAL_MIN, SteamAppState};

use crate::app_context::ConnectivityState;
use crate::game_view::{self, GameViewMessage};
use crate::messaging::{self, BannerSeverity, ToastKind};
use crate::profile_view::{self, types::ProfileViewMessage};
use crate::{App, Message, Screen, routing, settings_commands, splash_commands};

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
    app.context.user.avatar_handle = profile
        .as_ref()
        .and_then(|p| p.avatar_png_bytes.as_ref())
        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));
    if let Some(p) = &profile {
        app.context.user.account_id = p.steam_id.saturating_sub(STEAM_ID_64_INDIVIDUAL_MIN) as u32;
    }
    app.context.user.profile = profile;
    Task::none()
}

pub(crate) fn handle_messaging(app: &mut App, msg: messaging::MessagingEvent) -> Task<Message> {
    match msg {
        messaging::MessagingEvent::ToastTick => app.context.messaging.tick_toasts(),
        messaging::MessagingEvent::ToastHovered(id, hovered) => {
            app.context.messaging.set_toast_hovered(id, hovered);
        }
        messaging::MessagingEvent::ToastDismissed(id) => {
            app.context.messaging.dismiss_toast(id);
        }
        messaging::MessagingEvent::BannerDismissed(id) => {
            app.context.messaging.dismiss_banner(id);
        }
    }
    Task::none()
}

pub(crate) fn handle_settings_flush_tick(app: &mut App) -> Task<Message> {
    let Some(since) = app.context.settings_dirty_since else {
        return Task::none();
    };
    if since.elapsed() < Duration::from_millis(200) {
        return Task::none();
    }
    app.context.settings_dirty_since = None;
    let snapshot = app.context.settings.clone();
    settings_commands::write_settings(snapshot)
}

pub(crate) fn handle_settings_written(app: &mut App, result: Result<(), String>) -> Task<Message> {
    if let Err(e) = result {
        tracing::error!("settings: write error: {e}");
        app.context
            .messaging
            .push_toast(ToastKind::Error, "Could not save settings", None);
    }
    Task::none()
}

pub(crate) fn handle_retry_steam_connect(app: &mut App) -> Task<Message> {
    app.context.connectivity = ConnectivityState::default();
    app.context
        .messaging
        .dismiss_all_banners_by_severity(BannerSeverity::Warning);
    splash_commands::probe_steam_reconnect()
}

pub(crate) fn handle_focus_search(app: &App) -> Task<Message> {
    match &app.screen {
        Screen::ProfileView(_) => iced::widget::operation::focus(profile_view::library_search_id()),
        Screen::GameView(_) => iced::widget::operation::focus(game_view::achievement_search_id()),
    }
}

pub(crate) fn handle_toggle_theme(app: &mut App) -> Task<Message> {
    use crate::ui::theme::AppTheme;
    let new_theme = match app.context.settings.ui.theme {
        AppTheme::Dark => AppTheme::Light,
        AppTheme::Light => AppTheme::Dark,
    };
    app.context.update_settings(|s| s.ui.theme = new_theme);
    Task::none()
}

pub(crate) fn handle_steam_state_refreshed(
    app: &mut App,
    payload: Option<(HashMap<u32, SteamAppState>, Option<SystemTime>)>,
) -> Task<Message> {
    if let Some((state, mtime)) = payload {
        app.context.steam.app_state = state;
        app.context.steam.app_state_mtime = mtime;
    }
    Task::none()
}

pub(crate) fn handle_app_assets_loaded(
    app: &mut App,
    assets: HashMap<u32, AppLibraryAssets>,
) -> Task<Message> {
    tracing::info!(
        count = assets.len(),
        "app_assets: loaded library_assets_full hashes from appinfo.vdf"
    );
    app.context.steam.library_assets = assets;

    let pv = crate::routing::current_profile_view_state(&app.screen, &app.preserved_profile_state);
    let Some(pv) = pv else {
        return Task::none();
    };
    let pending_ids: Vec<u32> = pv
        .games
        .iter()
        .filter(|g| matches!(g.capsule, crate::profile_view::types::CapsuleAsset::Pending))
        .map(|g| g.app_id)
        .collect();
    if pending_ids.is_empty() {
        return Task::none();
    }
    tracing::info!(
        count = pending_ids.len(),
        "app_assets: re-spawning capsule fetch for pending games after assets ready"
    );
    let size = pv.capsule_size;
    crate::profile_view::spawn_capsule_queue(pending_ids, size, &app.context.steam.library_assets)
        .map(Message::ProfileView)
}
