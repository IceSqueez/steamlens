use std::collections::HashSet;

use iced::Task;

use steamlens_core::{STEAM_ID_64_INDIVIDUAL_MIN, UserProfile};

use crate::cache;
use crate::cache::migration;
use crate::messaging::BannerSeverity;
use crate::profile_view::types::ProfileViewMessage;
use crate::{App, Message, ProbeFailure, Screen, boot, steam_connectivity};

pub(crate) fn handle_probe_result(
    app: &mut App,
    result: Result<Box<steamlens_core::ProbedProfile>, ProbeFailure>,
) -> Task<Message> {
    app.boot.probe_done = true;
    match result {
        Ok(boxed) => {
            let profile = *boxed;
            app.context.connectivity.steam_running = Some(true);
            app.context.connectivity.user_logged_in = Some(true);
            app.context
                .messaging
                .dismiss_all_banners_by_severity(BannerSeverity::Warning);

            let new_account_id = profile.steam_id.saturating_sub(STEAM_ID_64_INDIVIDUAL_MIN) as u32;
            let previous_account_id = app.context.user.account_id;

            let user_switched = previous_account_id != 0 && previous_account_id != new_account_id;
            if user_switched {
                tracing::info!(
                    old = previous_account_id,
                    new = new_account_id,
                    "probe: user switched accounts, discarding previous user state"
                );
                app.context.user.profile = None;
                app.context.user.avatar_handle = None;
                app.context.user.steam_level = None;
            }

            app.context.user.account_id = new_account_id;
            app.context.user.avatar_handle = profile
                .avatar_image
                .as_ref()
                .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));

            if let Some(root) = profile.steam_root.clone() {
                app.context.user.steam_root = root;
            }

            let cached = cache::make_cached_profile(
                profile.steam_id,
                profile.nickname.clone(),
                profile.avatar_image.clone(),
                profile.steam_root.clone(),
                profile.steam_level,
            );
            app.context.user.steam_level = profile.steam_level;
            app.context.user.profile = Some(UserProfile {
                steam_id: profile.steam_id,
                nickname: profile.nickname,
                avatar_png_bytes: profile.avatar_image,
            });

            let last_saved = app.context.settings.last_user_account_id;
            if last_saved != Some(new_account_id) {
                app.context.update_settings(|s| {
                    s.last_user_account_id = Some(new_account_id);
                });
            }

            let mut tasks = vec![
                boot::spawn_steam_state_refresh(
                    app.context.user.steam_root.clone(),
                    app.context.user.account_id,
                    app.context.steam.app_state_mtime,
                ),
                cache::commands::write_profile_cache(new_account_id, cached),
                spawn_migrate_then_continue(
                    new_account_id,
                    profile.game_summaries,
                    app.context.no_ach_cache.clone(),
                ),
            ];
            if let crate::Screen::GameView(state) = &mut app.screen
                && state.cache_only
            {
                let app_id = state.app_id;
                state.cache_only = false;
                state.phase = crate::game_view::GameViewPhase::Connecting;
                state.achievements.clear();
                state.stats.clear();
                state.icon_handles.clear();
                state.reveal_queue.clear();
                tracing::info!(
                    app_id,
                    "probe success while in GameView: re-connecting worker for current game"
                );
                app.context
                    .capsules
                    .unavailable
                    .retain(|(id, _)| *id != app_id);
                app.context
                    .capsules
                    .handles
                    .retain(|(id, _), _| *id != app_id);
                tasks.push(crate::worker_drain::disconnect_worker(app));
                let worker = crate::steam_worker::SteamWorker::spawn();
                tasks.push(worker.dispatch(
                    crate::steam_worker::SteamRequest::ConnectWithApp(app_id),
                    Message::DiscardReply,
                ));
                app.context.worker.current = Some(worker);
                let portrait_assets = app
                    .context
                    .steam
                    .library_assets
                    .get(&app_id)
                    .cloned()
                    .unwrap_or_default();
                tasks.push(iced::Task::perform(
                    crate::capsule_cache::fetch_capsule(
                        app_id,
                        crate::capsule_cache::CapsuleSize::Portrait,
                        portrait_assets,
                    ),
                    move |result| match result {
                        Ok((size, pixels)) => {
                            let handle = iced::widget::image::Handle::from_rgba(
                                pixels.width,
                                pixels.height,
                                pixels.rgba,
                            );
                            Message::GameView(crate::game_view::GameViewMessage::CapsuleLoaded {
                                app_id,
                                size,
                                handle,
                                width: pixels.width,
                                height: pixels.height,
                            })
                        }
                        Err((size, _)) => {
                            Message::GameView(crate::game_view::GameViewMessage::CapsuleFailed {
                                app_id,
                                size,
                            })
                        }
                    },
                ));
            }
            Task::batch(tasks)
        }
        Err(ProbeFailure::NotLoggedIn) => {
            app.context.connectivity.steam_running = Some(true);
            app.context.connectivity.user_logged_in = Some(false);
            app.context.user.steam_level = None;
            tracing::warn!("probe: connectivity.user_logged_in = false");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotLoggedIn,
            );

            let account_id = fallback_account_id(app);
            classify_with_libcache_or_load(app, account_id)
        }
        Err(ProbeFailure::SteamNotRunning) => {
            app.context.connectivity.steam_running = Some(false);
            app.context.connectivity.user_logged_in = None;
            app.context.user.steam_level = None;
            tracing::warn!("probe: steam_running = false");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotRunning,
            );

            let account_id = fallback_account_id(app);
            classify_with_libcache_or_load(app, account_id)
        }
        Err(ProbeFailure::Other(reason)) => {
            app.context.connectivity.steam_running = None;
            app.context.connectivity.user_logged_in = None;
            app.context.user.steam_level = None;
            tracing::warn!("probe failed: {reason}");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotRunning,
            );

            let account_id = fallback_account_id(app);
            classify_with_libcache_or_load(app, account_id)
        }
    }
}

fn classify_with_libcache_or_load(app: &mut App, account_id: u32) -> Task<Message> {
    if let Screen::ProfileView(pv) = &app.screen
        && !pv.games.is_empty()
    {
        let summaries: Vec<steamlens_core::GameSummary> = pv
            .games
            .iter()
            .map(|g| steamlens_core::GameSummary {
                app_id: g.app_id,
                change_number: g.change_number,
                last_played: g.last_played,
            })
            .collect();
        let steam_root = app.context.user.steam_root.clone();
        tracing::info!(
            "probe fallback: libcache already loaded, classifying directly ({} games)",
            summaries.len()
        );
        app.boot.probe_classified = true;
        return Task::batch([
            cache::commands::load_profile_cache(account_id),
            cache::commands::classify_games(summaries, steam_root, account_id),
        ]);
    }
    Task::batch([
        cache::commands::load_profile_cache(account_id),
        cache::commands::load_library_cache(account_id),
    ])
}

pub(crate) fn handle_probe_library_ready(
    app: &mut App,
    account_id: u32,
    game_summaries: Vec<steamlens_core::GameSummary>,
    no_ach: cache::NoAchievementsCache,
) -> Task<Message> {
    app.context.no_ach_cache = no_ach;

    let pkginfo_count = game_summaries.len();
    tracing::info!("packageinfo: {pkginfo_count} games after type-filter");
    let no_ach = &app.context.no_ach_cache;
    let cache_entries = no_ach.entries.len();
    let game_view_app_id = match &app.screen {
        Screen::GameView(s) => Some(s.app_id),
        _ => None,
    };
    let filtered: Vec<_> = game_summaries
        .into_iter()
        .filter(|g| !no_ach.is_known_empty(g.app_id, g.change_number))
        .collect();
    let library_scan_summaries: Vec<_> = filtered
        .iter()
        .filter(|g| game_view_app_id.is_none_or(|id| g.app_id != id))
        .cloned()
        .collect();
    let total = filtered.len();
    let dropped = pkginfo_count.saturating_sub(total);
    tracing::info!(
        "no_ach: cache has {cache_entries} entries; filtered {dropped}/{pkginfo_count} pkginfo games; {total} remain for scan (game_view_excluded={})",
        game_view_app_id.is_some()
    );

    let steam_root = app.context.user.steam_root.clone();
    app.boot.probe_classified = true;

    let current_pv =
        crate::routing::current_profile_view_state(&app.screen, &app.preserved_profile_state);
    if app.boot.library_cache_resolved
        && let Some(pv) = current_pv
    {
        let probe_ids: HashSet<u32> = filtered.iter().map(|g| g.app_id).collect();
        let current_ids: HashSet<u32> = pv.games.iter().map(|g| g.app_id).collect();
        if probe_ids == current_ids {
            tracing::info!(
                "probe: library matches libcache ({} games); skipping ScanComplete, classifying with fresh probe data",
                probe_ids.len()
            );
            return cache::commands::classify_games(library_scan_summaries, steam_root, account_id);
        }
        tracing::info!(
            "probe: library diverges from libcache (libcache={}, probe={}); firing ScanComplete + classify with probe data (libcache discarded — pipe is authoritative)",
            current_ids.len(),
            probe_ids.len()
        );
    }

    let classify_task =
        cache::commands::classify_games(library_scan_summaries, steam_root, account_id);
    Task::batch([
        classify_task,
        Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
            filtered,
        ))),
    ])
}

fn fallback_account_id(app: &App) -> u32 {
    app.context
        .settings
        .last_user_account_id
        .unwrap_or(app.context.user.account_id)
}

fn spawn_migrate_then_continue(
    account_id: u32,
    game_summaries: Vec<steamlens_core::GameSummary>,
    no_ach: cache::NoAchievementsCache,
) -> Task<Message> {
    Task::perform(
        async move {
            match migration::migrate_legacy_cache_if_present(account_id).await {
                Ok(outcome) => tracing::trace!(account_id, ?outcome, "migration outcome"),
                Err(e) => tracing::warn!(account_id, error = %e, "migration error"),
            }
            (game_summaries, no_ach)
        },
        move |(summaries, no_ach)| Message::ProbeLibraryReady {
            account_id,
            summaries,
            no_ach,
        },
    )
}
