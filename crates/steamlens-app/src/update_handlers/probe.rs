use iced::Task;

use steamlens_core::{STEAMID64_INDIVIDUAL_MIN, UserProfile};

use crate::cache;
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
            app.context.steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
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

            let mut tasks: Vec<Task<Message>> = Vec::new();

            tasks.push(boot::spawn_steam_state_refresh(
                app.context.steam_root.clone(),
                app.context.steamid3,
                app.context.steam_state_mtime,
            ));

            tasks.push(cache::commands::write_profile_cache(cached));

            if !p.game_summaries.is_empty() {
                let pkginfo_count = p.game_summaries.len();
                tracing::info!("packageinfo: {pkginfo_count} games after type-filter");
                let no_ach = &app.context.no_ach_cache;
                let cache_entries = no_ach.entries.len();
                let filtered: Vec<_> = p
                    .game_summaries
                    .into_iter()
                    .filter(|g| !no_ach.is_known_empty(g.app_id, g.change_number))
                    .collect();
                let total = filtered.len();
                let dropped = pkginfo_count - total;
                tracing::info!(
                    "no_ach: cache has {cache_entries} entries; filtered {dropped}/{pkginfo_count} pkginfo games; {total} remain for scan"
                );
                let _ = total;
                tasks.push(Task::done(Message::ProfileView(
                    ProfileViewMessage::ScanComplete(filtered),
                )));
            } else {
                tasks.push(cache::commands::load_library_cache());
            }

            Task::batch(tasks)
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

            Task::batch([
                cache::commands::load_profile_cache(),
                cache::commands::load_library_cache(),
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

            Task::batch([
                cache::commands::load_profile_cache(),
                cache::commands::load_library_cache(),
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

            Task::batch([
                cache::commands::load_profile_cache(),
                cache::commands::load_library_cache(),
            ])
        }
    }
}
