use std::collections::HashMap;
use std::mem;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use iced::Task;
use iced::keyboard;

use steamlens_core::{STEAMID64_INDIVIDUAL_MIN, UserProfile};

use crate::cache::{self, CachedLibraryEntry, ClassifyResult};
use crate::game_view::{self, GameViewMessage};
use crate::game_view_seed;
use crate::messaging::{self, BannerSeverity, ToastKind};
use crate::profile_view::{self, types::ProfileEvent, types::ProfileViewMessage};
use crate::{App, Message, ProbeFailure, Screen, boot, routing, steam_connectivity};

pub(crate) fn handle_profile_view(app: &mut App, msg: ProfileViewMessage) -> Task<Message> {
    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);

    let is_scan_complete = matches!(msg, ProfileViewMessage::ScanComplete(_));
    let is_scan_failed = matches!(msg, ProfileViewMessage::ScanFailed { .. });
    let scan_failed_details = if let ProfileViewMessage::ScanFailed { app_id, ref reason } = msg {
        Some((app_id, reason.clone()))
    } else {
        None
    };
    let enumerated_games = if let ProfileViewMessage::ScanComplete(ref v) = msg {
        Some(v.clone())
    } else {
        None
    };

    let (task, event) = profile_view::update(pv_state, msg, &mut app.context);
    let task = task.map(Message::ProfileView);

    let extra = if is_scan_complete {
        app.boot.library_cache_resolved = true;
        tracing::info!("library_cache_resolved = true (ScanComplete)");
        let games = enumerated_games.unwrap_or_default();
        let steam_root = app.context.steam_root.clone();
        let steamid3 = app.context.steamid3;
        let classify_task = cache::commands::classify_games(games, steam_root, steamid3);

        let mut tasks: Vec<Task<Message>> = vec![classify_task, task];

        let pv_state =
            routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
        if !pv_state.library_name_map.is_empty() {
            let name_map = mem::take(&mut pv_state.library_name_map);
            for game in &mut pv_state.games {
                if let Some(name) = name_map.get(&game.app_id) {
                    game.name = Some(name.clone());
                }
            }
        }
        if !pv_state.games.is_empty() {
            let cached = cache::make_cached_library(
                pv_state
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
            tasks.push(cache::commands::write_library_cache(cached));
        }
        pv_state.recompute_derived(
            &app.context.cached_entries,
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
                    .cached_entries
                    .get(&app_id)
                    .map(|e| e.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("app {app_id}"));
                let action = messaging::ToastAction {
                    label: "Retry".to_owned(),
                    on_press: Message::ProfileView(ProfileViewMessage::RetrySingleFailedScan(
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
            let pin_task = app.context.update_settings(|s| {
                if let Some(pos) = s.library.pinned.iter().position(|&pid| pid == id) {
                    s.library.pinned.remove(pos);
                } else {
                    s.library.pinned.push(id);
                }
            });
            let pinned = app.context.settings.library.pinned.clone();
            let pv_state =
                routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
            pv_state.recompute_derived(&app.context.cached_entries, &pinned);
            Task::batch([extra, pin_task])
        }
        ProfileEvent::DrainedProgress {
            cache_entries,
            summary_entries,
            no_ach_entries,
        } => {
            let mut tasks: Vec<Task<Message>> = vec![extra];
            for entry in cache_entries {
                tasks.push(cache::commands::write_game_cache(entry));
            }
            for summary in summary_entries {
                tasks.push(cache::commands::write_game_summary(summary));
            }
            if !no_ach_entries.is_empty() {
                for (app_id, cn) in no_ach_entries {
                    app.context.no_ach_cache.insert(app_id, cn);
                }
                let snapshot = app.context.no_ach_cache.clone();
                tasks.push(cache::commands::write_no_ach_cache(snapshot));
            }
            Task::batch(tasks)
        }
    }
}

pub(crate) fn handle_cache_classified(app: &mut App, result: ClassifyResult) -> Task<Message> {
    app.boot.cache_classified = true;
    tracing::info!("cache_classified = true (CacheClassified)");

    let ClassifyResult {
        hits,
        dirty,
        schema_bumped,
        invalidation_count,
    } = result;

    let hit_count = hits.len();
    app.context.pending_hit_queue.extend(hits);

    let steam_off = app.context.connectivity.steam_running == Some(false);

    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);

    if !dirty.is_empty() && !steam_off {
        pv_state.scan_target_count = dirty.len();
        pv_state.scan_started_at = Some(Instant::now());
        pv_state.start_scan(dirty);
    } else {
        pv_state.last_scan_completed_at = Some(Instant::now());
    }
    let _ = hit_count;

    if invalidation_count > 0 {
        app.context.messaging.push_toast(
            ToastKind::Info,
            format!("{invalidation_count} games refreshing (cache invalidated)"),
            None,
        );
    }

    if schema_bumped > 0 {
        app.context.messaging.push_toast(
            ToastKind::Info,
            "Cache schema updated — refreshing library in the background".to_owned(),
            None,
        );
    }
    Task::none()
}

pub(crate) fn handle_drain_hit_queue(app: &mut App) -> Task<Message> {
    const HITS_PER_TICK: usize = 8;
    let mut touched = false;
    for _ in 0..HITS_PER_TICK {
        let Some(hit) = app.context.pending_hit_queue.pop_front() else {
            break;
        };
        let mut entry = hit.entry;
        game_view_seed::recompute_tier_breakdown_if_missing(&mut entry);
        if let Screen::ProfileView(pv_state) = &mut app.screen
            && let Some(game) = pv_state.games.iter_mut().find(|g| g.app_id == hit.app_id)
        {
            use crate::progress_scan::ProgressData;
            game.name = Some(entry.name.clone());
            game.progress = Some(ProgressData {
                earned: entry.progress.earned,
                total: entry.progress.total,
            });
            game.genre = entry.genre.clone();
        }
        app.context.cached_entries.insert(hit.app_id, entry);
        touched = true;
    }
    if touched {
        let pinned = app.context.settings.library.pinned.clone();
        let pv_state =
            routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
        pv_state.recompute_derived(&app.context.cached_entries, &pinned);
    }
    Task::none()
}

pub(crate) fn handle_persist_game_summary(app: &mut App, app_id: u32) -> Task<Message> {
    let Screen::GameView(gv_state) = &app.screen else {
        return Task::none();
    };
    if gv_state.app_id != app_id {
        return Task::none();
    }

    let earned = gv_state
        .achievements
        .iter()
        .filter(|a| a.effective_achieved())
        .count() as u32;
    let total = gv_state.achievements.len() as u32;

    let change_number = app
        .preserved_profile_state
        .as_ref()
        .and_then(|pv| {
            pv.games
                .iter()
                .find(|g| g.app_id == app_id)
                .map(|g| g.change_number)
        })
        .unwrap_or(0);

    let genre = app
        .context
        .cached_entries
        .get(&app_id)
        .and_then(|e| e.genre.clone());

    let name = gv_state.game_name.clone();
    let tier_breakdown = gv_state.tier_breakdown.clone();

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let playtime_minutes = app
        .context
        .steam_state
        .get(&app_id)
        .and_then(|s| s.playtime_minutes);

    let summary = cache::types::GameSummaryCache {
        schema_version: cache::types::SUMMARY_SCHEMA_VERSION,
        app_id,
        name,
        cached_change_number: change_number,
        cached_at: now_secs,
        progress: cache::types::CachedProgress { earned, total },
        tier_breakdown,
        genre,
        playtime_minutes,
    };

    if let Some(existing) = app.context.cached_entries.get(&app_id)
        && existing.progress.earned == summary.progress.earned
        && existing.progress.total == summary.progress.total
        && existing.tier_breakdown == summary.tier_breakdown
        && existing.playtime_minutes == summary.playtime_minutes
    {
        return Task::none();
    }

    tracing::info!(app_id, earned, total, change_number, "persist game summary");

    let mut full_entry =
        game_view_seed::build_game_view_cache_entry(gv_state, app_id, &app.context.steam_state);
    if let Some(existing) = app.context.cached_entries.get(&app_id) {
        cache::store::merge_preserved_fields(&mut full_entry, existing);
    }
    app.context
        .cached_entries
        .insert(app_id, full_entry.clone());

    let pinned = app.context.settings.library.pinned.clone();
    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
    pv_state.recompute_derived(&app.context.cached_entries, &pinned);

    let Screen::GameView(gv_state) = &app.screen else {
        return Task::none();
    };
    let icons_to_write: Vec<(String, steamlens_core::AchievementIcon)> = gv_state
        .achievements
        .iter()
        .filter_map(|r| r.data.icon.as_ref().map(|i| (r.data.id.clone(), i.clone())))
        .collect();
    let icons_task = Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                for (id, icon) in &icons_to_write {
                    if let Err(e) = cache::icons::write_blocking(app_id, id, icon) {
                        tracing::warn!("icon cache write failed app_id={app_id} ach={id}: {e}");
                    }
                }
            })
            .await
            .map_err(|e| e.to_string())
        },
        move |result| {
            Message::Cache(cache::CacheEvent::GameWritten {
                app_id,
                result: result.map(|_| ()),
            })
        },
    );

    let summary_task = Task::perform(
        async move { cache::store::write_game_summary(&summary).await },
        move |result| {
            Message::Cache(cache::CacheEvent::GameWritten {
                app_id,
                result: result.map_err(|e| e.to_string()),
            })
        },
    );
    let game_task = cache::commands::write_game_cache(full_entry);

    Task::batch([summary_task, game_task, icons_task])
}

pub(crate) fn handle_invalidate_game_cache(app: &mut App, app_id: u32) -> Task<Message> {
    let name = app
        .context
        .cached_entries
        .get(&app_id)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| format!("App {app_id}"));
    app.context.cached_entries.remove(&app_id);

    let steam_on = app.context.connectivity.steam_running == Some(true);

    let pinned = app.context.settings.library.pinned.clone();
    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
    if let Some(entry) = pv_state.games.iter_mut().find(|g| g.app_id == app_id) {
        entry.progress = None;
        entry.capsule = profile_view::types::CapsuleAsset::Pending;
    }
    pv_state.capsule_handles.retain(|(id, _), _| *id != app_id);
    if steam_on {
        pv_state.start_scan(vec![app_id]);
    }
    pv_state.recompute_derived(&app.context.cached_entries, &pinned);

    cache::commands::invalidate_game_cache(app_id, name)
}

pub(crate) fn handle_game_invalidated(
    app: &mut App,
    app_id: u32,
    name: String,
    result: Result<(), String>,
) -> Task<Message> {
    match result {
        Ok(()) => app.context.messaging.push_toast(
            ToastKind::Success,
            format!("Cache cleared for {name}"),
            None,
        ),
        Err(e) => {
            tracing::error!(error = %e, %name, "cache invalidate failed");
            app.context.messaging.push_toast(
                ToastKind::Error,
                format!("Failed to clear cache for {name}"),
                None,
            );
            return Task::none();
        }
    }
    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
    let size = pv_state.capsule_size;
    profile_view::spawn_capsule_queue(vec![app_id], size, &app.context.app_assets)
        .map(Message::ProfileView)
}

pub(crate) fn handle_probe_result(
    app: &mut App,
    result: Result<steamlens_core::ProbedProfile, ProbeFailure>,
) -> Task<Message> {
    app.boot.probe_done = true;
    match result {
        Ok(p) => {
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

pub(crate) fn handle_profile_loaded(
    app: &mut App,
    maybe: Option<cache::CachedProfile>,
) -> Task<Message> {
    let Some(cached) = maybe else {
        return Task::none();
    };
    if app.context.user_profile.is_some()
        && app.context.connectivity.steam_running != Some(false)
        && app.context.connectivity.user_logged_in != Some(false)
    {
        return Task::none();
    }
    app.context.steamid3 = cached.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
    app.context.steam_level = cached.steam_level;
    app.context.profile_avatar_handle = cached
        .avatar_png_bytes
        .as_ref()
        .map(|bytes| iced::widget::image::Handle::from_bytes(bytes.clone()));
    app.context.user_profile = Some(UserProfile {
        steam_id: cached.steam_id,
        nickname: cached.nickname,
        avatar_png_bytes: cached.avatar_png_bytes,
    });
    Task::none()
}

pub(crate) fn handle_library_loaded(
    app: &mut App,
    maybe: Option<cache::CachedLibrary>,
) -> Task<Message> {
    let games_present = if let Screen::ProfileView(pv) = &app.screen {
        !pv.games.is_empty()
    } else {
        true
    };
    if games_present {
        return Task::none();
    }
    let Some(cached) = maybe else {
        return Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
            Vec::new(),
        )));
    };
    let summary: Vec<steamlens_core::GameSummary> = cached
        .games
        .iter()
        .map(|e| steamlens_core::GameSummary {
            app_id: e.app_id,
            change_number: e.change_number,
            last_played: e.last_played,
        })
        .collect();
    let name_map: HashMap<u32, String> = cached
        .games
        .into_iter()
        .filter(|e| !e.name.is_empty())
        .map(|e| (e.app_id, e.name))
        .collect();
    if let Screen::ProfileView(pv_state) = &mut app.screen {
        pv_state.library_name_map = name_map;
    }
    app.boot.library_cache_resolved = true;
    tracing::info!("library_cache_resolved = true (LibraryCacheLoaded)");
    Task::done(Message::ProfileView(ProfileViewMessage::ScanComplete(
        summary,
    )))
}

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

pub(crate) fn handle_game_view_message(app: &mut App, m: GameViewMessage) -> Task<Message> {
    let Screen::GameView(state) = &mut app.screen else {
        #[cfg(debug_assertions)]
        tracing::warn!("dropped stale GameView message: {m:?} (current screen: not GameView)");
        return Task::none();
    };

    let (task, event) = game_view::update(state, m, &mut app.context);
    let task = task.map(Message::GameView);
    routing::dispatch_game_event(app, task, event)
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

pub(crate) fn handle_game_sort_changed(
    app: &mut App,
    sort: crate::game_view::types::AchievementSort,
) -> Task<Message> {
    let Screen::GameView(state) = &mut app.screen else {
        return Task::none();
    };
    let (task, _event) = game_view::update(
        state,
        GameViewMessage::AchievementSortChanged(sort),
        &mut app.context,
    );
    task.map(Message::GameView)
}

pub(crate) fn handle_update_check_result(
    app: &mut App,
    result: Result<Option<crate::update_check::UpdateInfo>, String>,
) -> Task<Message> {
    match result {
        Ok(Some(info)) => {
            let body = format!(
                "A new version {} is available \u{2014} click Download to get it.",
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
        app.context.steamid3 = p.steam_id.saturating_sub(STEAMID64_INDIVIDUAL_MIN);
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

pub(crate) fn handle_offline_loaded(
    app: &mut App,
    app_id: u32,
    entry: Option<Box<cache::GameCacheEntry>>,
) -> Task<Message> {
    let Some(full) = entry.map(|b| *b) else {
        if let Screen::GameView(state) = &mut app.screen
            && state.app_id == app_id
            && state.cache_only
        {
            state.phase = game_view::GameViewPhase::Error;
            state.error_message =
                "No cached data \u{2014} reconnect Steam to load this game".to_owned();
        }
        return Task::none();
    };
    if let Screen::GameView(state) = &mut app.screen
        && state.app_id == app_id
    {
        state.expected_total = full.progress.total;
        if state.genre.is_none() {
            state.genre = full.genre.clone();
        }
        if state.playtime_minutes.is_none() {
            state.playtime_minutes = full.playtime_minutes;
        }
    }
    let seed_task = if full.achievements.is_empty() {
        Task::none()
    } else {
        game_view_seed::spawn_seed_task(app_id, full.clone())
    };
    app.context.cached_entries.insert(app_id, full);
    seed_task
}
