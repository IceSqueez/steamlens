use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use iced::Task;

use steamlens_core::{STEAMID64_INDIVIDUAL_MIN, UserProfile};

use crate::cache::{self, ClassifyResult};
use crate::game_view;
use crate::game_view_seed;
use crate::messaging::ToastKind;
use crate::profile_view::{self, types::ProfileViewMessage};
use crate::{App, Message, Screen, routing};

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
    const HITS_PER_TICK: usize = 32;
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
        const RECOMPUTE_DEBOUNCE: Duration = Duration::from_millis(250);
        let now = Instant::now();
        let queue_empty = app.context.pending_hit_queue.is_empty();
        let due = app
            .context
            .last_hit_recompute_at
            .is_none_or(|t| now.duration_since(t) >= RECOMPUTE_DEBOUNCE);
        if queue_empty || due {
            let pinned = app.context.settings.library.pinned.clone();
            let pv_state =
                routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
            pv_state.rebuild_available_genres();
            pv_state.recompute_derived(&app.context.cached_entries, &pinned);
            app.context.last_hit_recompute_at = Some(now);
        }
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

    app.context
        .capsule_handles
        .retain(|(id, _), _| *id != app_id);
    app.context
        .capsule_unavailable
        .retain(|(id, _)| *id != app_id);

    let pv_state = routing::current_pv_state_mut(&mut app.screen, &mut app.preserved_profile_state);
    if let Some(entry) = pv_state.games.iter_mut().find(|g| g.app_id == app_id) {
        entry.progress = None;
        entry.capsule = profile_view::types::CapsuleAsset::Pending;
    }
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
