use iced::Task;

use steamlens_core::{STEAMID64_INDIVIDUAL_MIN, UserProfile};

use crate::cache;
use crate::cache::migration;
use crate::messaging::BannerSeverity;
use crate::profile_view::types::ProfileViewMessage;
use crate::{App, Message, ProbeFailure, boot, steam_connectivity};

pub(crate) fn handle_probe_result(
    app: &mut App,
    result: Result<Box<steamlens_core::ProbedProfile>, ProbeFailure>,
) -> Task<Message> {
    app.boot.probe_done = true;
    match result {
        Ok(p) => {
            let p = *p;
            app.context.connectivity.steam_running = Some(true);
            app.context.connectivity.user_logged_in = Some(true);
            app.context
                .messaging
                .dismiss_all_banners_by_severity(BannerSeverity::Warning);

            let new_steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN) as u32;
            let previous_steamid3 = app.context.steamid3 as u32;

            let user_switched = previous_steamid3 != 0 && previous_steamid3 != new_steamid3;
            if user_switched {
                tracing::info!(
                    old = previous_steamid3,
                    new = new_steamid3,
                    "probe: user switched accounts, discarding previous user state"
                );
                app.context.user_profile = None;
                app.context.profile_avatar_handle = None;
                app.context.steam_level = None;
            }

            app.context.steamid3 = new_steamid3 as u64;
            app.context.profile_avatar_handle = p
                .avatar_image
                .as_ref()
                .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));

            if let Some(root) = p.steam_root.clone() {
                app.context.steam_root = root;
            }

            let cached = cache::make_cached_profile(
                p.steam_id,
                p.nickname.clone(),
                p.avatar_image.clone(),
                p.steam_root.clone(),
                p.steam_level,
            );
            app.context.steam_level = p.steam_level;
            app.context.user_profile = Some(UserProfile {
                steam_id: p.steam_id,
                nickname: p.nickname,
                avatar_png_bytes: p.avatar_image,
            });

            let last_saved = app.context.settings.last_user_steamid;
            if last_saved != Some(new_steamid3) {
                app.context.update_settings(|s| {
                    s.last_user_steamid = Some(new_steamid3);
                });
            }

            Task::batch(vec![
                boot::spawn_steam_state_refresh(
                    app.context.steam_root.clone(),
                    app.context.steamid3,
                    app.context.steam_state_mtime,
                ),
                cache::commands::write_profile_cache(new_steamid3, cached),
                spawn_migrate_then_continue(
                    new_steamid3,
                    p.game_summaries,
                    app.context.no_ach_cache.clone(),
                ),
            ])
        }
        Err(ProbeFailure::NotLoggedIn) => {
            app.context.connectivity.steam_running = Some(true);
            app.context.connectivity.user_logged_in = Some(false);
            app.context.steam_level = None;
            tracing::warn!("probe: connectivity.user_logged_in = false");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotLoggedIn,
            );

            let steamid3 = fallback_steamid3(app);
            Task::batch([
                cache::commands::load_profile_cache(steamid3),
                cache::commands::load_library_cache(steamid3),
            ])
        }
        Err(ProbeFailure::SteamNotRunning) => {
            app.context.connectivity.steam_running = Some(false);
            app.context.connectivity.user_logged_in = None;
            app.context.steam_level = None;
            tracing::warn!("probe: steam_running = false");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotRunning,
            );

            let steamid3 = fallback_steamid3(app);
            Task::batch([
                cache::commands::load_profile_cache(steamid3),
                cache::commands::load_library_cache(steamid3),
            ])
        }
        Err(ProbeFailure::Other(reason)) => {
            app.context.connectivity.steam_running = None;
            app.context.connectivity.user_logged_in = None;
            app.context.steam_level = None;
            tracing::warn!("probe failed: {reason}");

            steam_connectivity::surface_steam_unavailable(
                &mut app.context,
                steam_connectivity::SteamUnavailable::NotRunning,
            );

            let steamid3 = fallback_steamid3(app);
            Task::batch([
                cache::commands::load_profile_cache(steamid3),
                cache::commands::load_library_cache(steamid3),
            ])
        }
    }
}

pub(crate) fn handle_probe_library_ready(
    app: &mut App,
    steamid3: u32,
    game_summaries: Vec<steamlens_core::GameSummary>,
    no_ach: cache::NoAchievementsCache,
) -> Task<Message> {
    app.context.no_ach_cache = no_ach;

    if !game_summaries.is_empty() {
        let pkginfo_count = game_summaries.len();
        tracing::info!("packageinfo: {pkginfo_count} games after type-filter");
        let no_ach = &app.context.no_ach_cache;
        let cache_entries = no_ach.entries.len();
        let filtered: Vec<_> = game_summaries
            .into_iter()
            .filter(|g| !no_ach.is_known_empty(g.app_id, g.change_number))
            .collect();
        let total = filtered.len();
        let dropped = pkginfo_count - total;
        tracing::info!(
            "no_ach: cache has {cache_entries} entries; filtered {dropped}/{pkginfo_count} pkginfo games; {total} remain for scan"
        );
        let _ = total;
        Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
            filtered,
        )))
    } else {
        cache::commands::load_library_cache(steamid3)
    }
}

fn fallback_steamid3(app: &App) -> u32 {
    app.context
        .settings
        .last_user_steamid
        .unwrap_or(app.context.steamid3 as u32)
}

fn spawn_migrate_then_continue(
    steamid3: u32,
    game_summaries: Vec<steamlens_core::GameSummary>,
    no_ach: cache::NoAchievementsCache,
) -> Task<Message> {
    Task::perform(
        async move {
            match migration::migrate_legacy_cache_if_present(steamid3).await {
                Ok(outcome) => tracing::trace!(steamid3, ?outcome, "migration outcome"),
                Err(e) => tracing::warn!(steamid3, error = %e, "migration error"),
            }
            (game_summaries, no_ach)
        },
        move |(summaries, no_ach)| Message::ProbeLibraryReady {
            steamid3,
            summaries,
            no_ach,
        },
    )
}
