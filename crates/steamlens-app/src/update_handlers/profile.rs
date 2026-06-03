use std::mem;

use iced::Task;

use crate::cache::{self, CachedLibraryEntry};
use crate::messaging;
use crate::profile_view::{self, types::ProfileEvent, types::ProfileViewMessage};
use crate::{App, Message, routing, steam_connectivity};

pub(crate) fn handle_profile_view(app: &mut App, message: ProfileViewMessage) -> Task<Message> {
    let profile_view_state =
        routing::current_profile_view_state_mut(&mut app.screen, &mut app.preserved_profile_state);

    let is_scan_complete = matches!(message, ProfileViewMessage::ScanComplete(_));
    let is_scan_failed = matches!(message, ProfileViewMessage::ScanFailed { .. });
    let scan_failed_details = if let ProfileViewMessage::ScanFailed { app_id, ref reason } = message
    {
        Some((app_id, reason.clone()))
    } else {
        None
    };
    let enumerated_games = if let ProfileViewMessage::ScanComplete(ref v) = message {
        Some(v.clone())
    } else {
        None
    };

    let (task, event) = profile_view::update(profile_view_state, message, &mut app.context);
    let task = task.map(Message::ProfileView);

    let extra = if is_scan_complete {
        app.boot.library_cache_resolved = true;
        tracing::info!("library_cache_resolved = true (ScanComplete)");
        let games = enumerated_games.unwrap_or_default();
        let steam_root = app.context.user.steam_root.clone();
        let account_id = app.context.user.account_id;
        let classify_task = cache::commands::classify_games(games, steam_root, account_id);

        let mut tasks: Vec<Task<Message>> = vec![classify_task, task];

        let profile_view_state = routing::current_profile_view_state_mut(
            &mut app.screen,
            &mut app.preserved_profile_state,
        );
        if !profile_view_state.library_name_map.is_empty() {
            let name_map = mem::take(&mut profile_view_state.library_name_map);
            for game in &mut profile_view_state.games {
                if let Some(name) = name_map.get(&game.app_id) {
                    game.name = Some(name.clone());
                }
            }
        }
        if !profile_view_state.games.is_empty() {
            let cached = cache::make_cached_library(
                profile_view_state
                    .games
                    .iter()
                    .map(|g| CachedLibraryEntry {
                        app_id: g.app_id,
                        change_number: g.change_number,
                        last_played: g.last_played,
                        name: String::new(),
                        achievement_count: 0,
                    })
                    .collect(),
            );
            let account_id = app.context.user.account_id;
            tasks.push(cache::commands::write_library_cache(account_id, cached));
        }
        profile_view_state.recompute_derived(
            &app.context.game_cache.entries,
            &app.context.settings.library.pinned,
        );

        Task::batch(tasks)
    } else if is_scan_failed {
        if let Some((app_id, reason)) = scan_failed_details {
            if steam_connectivity::looks_like_steam_died(&reason) {
                steam_connectivity::mark_steam_offline_and_warn(app);
            } else {
                let name = app
                    .context
                    .game_cache
                    .entries
                    .get(&app_id)
                    .map(|e| e.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("app {app_id}"));
                let action = messaging::ToastAction {
                    label: "Retry".to_owned(),
                    on_press: Message::ProfileView(ProfileViewMessage::SingleScanRetryRequested(
                        app_id,
                    )),
                };
                app.context.messaging.push_toast_with_action(
                    messaging::ToastKind::Error,
                    format!("Failed to load {name}"),
                    Some(reason),
                    action,
                );
            }
        }
        task
    } else {
        task
    };

    match event {
        ProfileEvent::None => extra,
        ProfileEvent::OpenGame(app_id) => {
            let open_task = routing::open_game_view(app, app_id);
            Task::batch([extra, open_task])
        }
        ProfileEvent::ToggleGamePin(id) => {
            app.context.update_settings(|s| {
                if let Some(pos) = s.library.pinned.iter().position(|&pid| pid == id) {
                    s.library.pinned.remove(pos);
                } else {
                    s.library.pinned.push(id);
                }
            });
            let pinned = app.context.settings.library.pinned.clone();
            let profile_view_state = routing::current_profile_view_state_mut(
                &mut app.screen,
                &mut app.preserved_profile_state,
            );
            profile_view_state.recompute_derived(&app.context.game_cache.entries, &pinned);
            extra
        }
        ProfileEvent::DrainedProgress {
            cache_entries,
            summary_entries,
            no_ach_entries,
        } => {
            let mut tasks: Vec<Task<Message>> = vec![extra];
            let account_id = app.context.user.account_id;
            for entry in cache_entries {
                tasks.push(cache::commands::write_game_cache(account_id, entry));
            }
            for summary in summary_entries {
                tasks.push(cache::commands::write_game_summary(account_id, summary));
            }
            if !no_ach_entries.is_empty() {
                for (app_id, change_number) in no_ach_entries {
                    app.context.no_ach_cache.insert(app_id, change_number);
                }
                let snapshot = app.context.no_ach_cache.clone();
                tasks.push(cache::commands::write_no_ach_cache(snapshot));
            }
            Task::batch(tasks)
        }
    }
}
