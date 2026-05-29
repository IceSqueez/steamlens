use std::collections::HashMap;

use iced::Task;

use super::messages::{GameViewEvent, GameViewMessage};
use super::state::{GameViewPhase, GameViewState, compute_tier_breakdown};
use super::types::{AchievementRow, BulkOp, StatRow, StatValue, build_apply_payload};
use crate::steam_worker::{SteamReply, SteamRequest};

fn surface_connectivity_error(
    ctx: &mut crate::app_context::AppContext,
    err: crate::worker_subprocess::ConnectivityError,
) {
    use crate::worker_subprocess::ConnectivityError as CE;
    let msg = match err {
        CE::SteamNotRunning => "Steam disconnected — action skipped",
        CE::NotLoggedIn => "Not signed in to Steam — action skipped",
    };
    ctx.messaging
        .push_toast(crate::messaging::ToastKind::Error, msg.to_owned(), None);
}

pub fn handle_steam_reply(
    state: &mut GameViewState,
    reply: SteamReply,
    ctx: &mut crate::app_context::AppContext,
) -> Task<GameViewMessage> {
    match reply {
        SteamReply::Connected { app_name, .. } => {
            if let Some(name) = app_name {
                state.game_name = name;
            }
            state.phase = GameViewPhase::WaitingStats;
            Task::none()
        }
        SteamReply::ConnectFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::RequestStatsFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::AchievementsAndStats {
            achievements,
            stats,
        } => {
            let mut existing_icons: HashMap<String, steamlens_core::AchievementIcon> =
                HashMap::new();
            let mut existing_rarity_pct: HashMap<String, f32> = HashMap::new();
            let mut prev_revealed: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for row in state.achievements.drain(..) {
                if row.revealed {
                    prev_revealed.insert(row.data.id.clone());
                }
                if let Some(pct) = row.rarity_percent {
                    existing_rarity_pct.insert(row.data.id.clone(), pct);
                }
                if let Some(icon) = row.data.icon {
                    existing_icons.insert(row.data.id, icon);
                }
            }
            let pending_icons = std::mem::take(&mut state.pending_icons);
            let pending_pct = state.pending_rarity_percent.take();
            state.achievements = achievements
                .into_iter()
                .map(|mut data| {
                    if data.icon.is_none() {
                        data.icon = pending_icons
                            .get(&data.id)
                            .cloned()
                            .or_else(|| existing_icons.remove(&data.id));
                    }
                    let mut row = AchievementRow::from(data);
                    if prev_revealed.contains(&row.data.id) {
                        row.revealed = true;
                    }
                    row.rarity_percent = pending_pct
                        .as_ref()
                        .and_then(|m| m.get(&row.data.id).copied())
                        .or_else(|| existing_rarity_pct.get(&row.data.id).copied());
                    row
                })
                .collect();
            state.stats = stats.into_iter().map(StatRow::from).collect();
            state.phase = GameViewPhase::Ready;
            state.fade_in = 0.0;

            state.reveal_queue = state
                .achievements
                .iter()
                .map(|r| r.data.id.clone())
                .collect();

            state.tier_breakdown = compute_tier_breakdown(&state.achievements);
            state.recompute_derived();

            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::LoadFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::ChangesSaved => {
            for row in &mut state.achievements {
                if row.is_dirty {
                    row.data.is_achieved = row.effective_achieved();
                    row.is_dirty = false;
                }
            }
            for row in &mut state.stats {
                if row.is_dirty {
                    row.data.original_value = row.data.value;
                    row.is_dirty = false;
                }
            }
            state.phase = GameViewPhase::Ready;
            state.recompute_derived();
            ctx.messaging.push_toast(
                crate::messaging::ToastKind::Success,
                "Changes saved to Steam".to_owned(),
                None,
            );
            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::SaveFailed(e) => {
            state.phase = GameViewPhase::Ready;
            ctx.messaging.push_toast(
                crate::messaging::ToastKind::Error,
                format!("Failed to save: {e}"),
                None,
            );
            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::IconUpdated { name, icon } => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == name) {
                row.data.icon = Some(icon);
            } else {
                state.pending_icons.insert(name, icon);
            }
            Task::none()
        }
        SteamReply::Disconnected => Task::none(),
        SteamReply::GlobalPercentagesReady(map) => {
            if state.achievements.is_empty() {
                state.pending_rarity_percent = Some(map);
                Task::none()
            } else {
                for row in &mut state.achievements {
                    if let Some(&pct) = map.get(&row.data.id) {
                        row.rarity_percent = Some(pct);
                    }
                }
                state.tier_breakdown = compute_tier_breakdown(&state.achievements);
                state.recompute_derived();
                Task::done(GameViewMessage::AchievementsFullyLoaded)
            }
        }
        SteamReply::GlobalPercentagesFailed => Task::none(),
    }
}

pub fn update(
    state: &mut GameViewState,
    message: GameViewMessage,
    ctx: &mut crate::app_context::AppContext,
) -> (Task<GameViewMessage>, GameViewEvent) {
    let worker = ctx.worker.as_ref();
    match message {
        GameViewMessage::Noop => (Task::none(), GameViewEvent::None),

        GameViewMessage::AchievementToggled(id) => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id) {
                if row.data.permission != 0 {
                    ctx.messaging.push_toast(
                        crate::messaging::ToastKind::Info,
                        "This achievement is protected and cannot be modified".to_owned(),
                        None,
                    );
                } else {
                    row.is_dirty = !row.is_dirty;
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::FilterChanged(f) => {
            state.filter = f;
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RarityTierToggled(tier) => {
            if state.rarity_tier_set.contains(&tier) {
                state.rarity_tier_set.remove(&tier);
            } else {
                state.rarity_tier_set.insert(tier);
            }
            let tiers: Vec<_> = state.rarity_tier_set.iter().copied().collect();
            let include_hidden = state.include_hidden;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = tiers;
                s.manager.include_hidden = include_hidden;
            });
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::HiddenPillToggled => {
            state.include_hidden = !state.include_hidden;
            let tiers: Vec<_> = state.rarity_tier_set.iter().copied().collect();
            let include_hidden = state.include_hidden;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = tiers;
                s.manager.include_hidden = include_hidden;
            });
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RarityFilterCleared => {
            state.rarity_tier_set.clear();
            state.include_hidden = false;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = Vec::new();
                s.manager.include_hidden = false;
            });
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::AchievementSortChanged(s) => {
            let sort = s;
            let _ = ctx.update_settings(|s| s.manager.sort = sort);
            state.achievement_sort = s;
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::SearchChanged(q) => {
            state.search_query = q;
            state.recompute_visible_only();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsSearchChanged(q) => {
            state.stats_search_query = q;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsMaxAll => {
            for stat in &mut state.stats {
                let Some(max) = stat.data.max_value else {
                    continue;
                };
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(max as i32),
                    StatValue::Float(_) => StatValue::Float(max as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsResetAll => {
            for stat in &mut state.stats {
                let default = stat.data.default_value.unwrap_or(0);
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(default as i32),
                    StatValue::Float(_) => StatValue::Float(default as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsResetSingle(id) => {
            if let Some(stat) = state.stats.iter_mut().find(|s| s.data.id == id) {
                let default = stat.data.default_value.unwrap_or(0);
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(default as i32),
                    StatValue::Float(_) => StatValue::Float(default as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsMaxSingle(id) => {
            if let Some(stat) = state.stats.iter_mut().find(|s| s.data.id == id)
                && let Some(max) = stat.data.max_value
            {
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(max as i32),
                    StatValue::Float(_) => StatValue::Float(max as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::BulkAction(op) => {
            let visible: std::collections::HashSet<usize> =
                state.derived.visible_indices.iter().copied().collect();

            for (i, row) in state.achievements.iter_mut().enumerate() {
                if row.data.permission != 0 {
                    continue;
                }
                if !visible.contains(&i) {
                    continue;
                }
                match op {
                    BulkOp::Unlock => {
                        row.is_dirty = !row.data.is_achieved;
                    }
                    BulkOp::Lock => {
                        row.is_dirty = row.data.is_achieved;
                    }
                    BulkOp::Invert => {
                        row.is_dirty = !row.is_dirty;
                    }
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ReloadRequested => {
            state.phase = GameViewPhase::WaitingStats;
            state.achievements.clear();
            state.stats.clear();
            state.reveal_queue.clear();
            state.recompute_derived();
            let mut tasks: Vec<Task<GameViewMessage>> = Vec::new();
            if let Some(w) = worker {
                let steam_running = ctx.connectivity.steam_running.unwrap_or(false);
                let user_logged_in = ctx.connectivity.user_logged_in.unwrap_or(false);
                match w.dispatch_checked(
                    SteamRequest::RequestUserStats,
                    steam_running,
                    user_logged_in,
                    GameViewMessage::Noop,
                ) {
                    Ok(t) => tasks.push(t),
                    Err(e) => {
                        surface_connectivity_error(ctx, e);
                        return (Task::none(), GameViewEvent::None);
                    }
                }
                match w.dispatch_checked(
                    SteamRequest::RequestGlobalPercentages,
                    steam_running,
                    user_logged_in,
                    GameViewMessage::Noop,
                ) {
                    Ok(t) => tasks.push(t),
                    Err(e) => {
                        surface_connectivity_error(ctx, e);
                        return (Task::none(), GameViewEvent::None);
                    }
                }
            }
            (Task::batch(tasks), GameViewEvent::None)
        }
        GameViewMessage::ApplyClicked => {
            if state.cache_only || state.dirty_count() == 0 || state.has_stat_errors() {
                return (Task::none(), GameViewEvent::None);
            }
            state.apply_confirm_input.clear();
            state.show_apply_modal = true;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyConfirmInputChanged(text) => {
            state.apply_confirm_input = text;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyCancelled => {
            state.show_apply_modal = false;
            state.apply_confirm_input.clear();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyConfirmed => {
            if !state.apply_confirm_matches() {
                return (Task::none(), GameViewEvent::None);
            }
            state.show_apply_modal = false;
            state.apply_confirm_input.clear();
            if state.dirty_count() == 0 || state.has_stat_errors() {
                return (Task::none(), GameViewEvent::None);
            }
            let payload = build_apply_payload(&state.achievements, &state.stats);
            state.phase = GameViewPhase::Saving;
            if let Some(w) = worker {
                let steam_running = ctx.connectivity.steam_running.unwrap_or(false);
                let user_logged_in = ctx.connectivity.user_logged_in.unwrap_or(false);
                match w.dispatch_checked(
                    SteamRequest::ApplyChanges {
                        achievements_to_set: payload.achievements_to_set,
                        achievements_to_clear: payload.achievements_to_clear,
                        stats_int: payload.stats_int,
                        stats_float: payload.stats_float,
                    },
                    steam_running,
                    user_logged_in,
                    GameViewMessage::Noop,
                ) {
                    Ok(t) => return (t, GameViewEvent::None),
                    Err(e) => {
                        surface_connectivity_error(ctx, e);
                        state.phase = GameViewPhase::Ready;
                        return (
                            Task::none(),
                            GameViewEvent::AchievementsFullyLoaded {
                                app_id: state.app_id,
                            },
                        );
                    }
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::DiscardChanges => {
            for row in &mut state.achievements {
                row.is_dirty = false;
            }
            for row in &mut state.stats {
                row.data.value = row.data.original_value;
                row.edit_text = row.data.original_value.to_edit_string();
                row.is_dirty = false;
                row.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RevealHidden(id) => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id) {
                row.revealed = true;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::AchievementsFullyLoaded => (
            Task::none(),
            GameViewEvent::AchievementsFullyLoaded {
                app_id: state.app_id,
            },
        ),
        GameViewMessage::RequestGoBack => (Task::none(), GameViewEvent::GoBack),
        GameViewMessage::CapsuleLoaded {
            app_id,
            size,
            handle,
            width,
            height,
        } => {
            ctx.capsule_handles.insert(
                (app_id, size),
                crate::profile_view::types::StoredCapsule {
                    handle,
                    width,
                    height,
                },
            );
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::CapsuleFailed { app_id, size } => {
            tracing::warn!("game_view: capsule fetch failed for app_id={app_id} size={size:?}");
            ctx.capsule_unavailable.insert((app_id, size));
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::BarSliceHoverEnter(tier) => {
            state.hovered_bar_slice = Some(tier);
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::BarSliceHoverExit => {
            state.hovered_bar_slice = None;
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::InvalidateCacheClicked(app_id) => {
            (Task::none(), GameViewEvent::InvalidateCache { app_id })
        }

        GameViewMessage::CacheSeeded { app_id, seeded } => {
            if state.app_id == app_id {
                state.game_name = seeded.game_name;
                state.achievements = seeded.achievements;
                state.stats = seeded.stats;
                state.reveal_queue.clear();
                state.recompute_derived();
            }
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::AchievementGridScrolled(y) => {
            state.achievement_grid_scroll_y = y;
            (Task::none(), GameViewEvent::None)
        }
    }
}
