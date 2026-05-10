pub mod profile;
pub mod types;
mod view;

pub use view::ProfileViewProps;
pub use view::library_search_id;

pub fn header_content<'a>(
    state: &'a types::ProfileViewState,
    _steam_running: Option<bool>,
) -> crate::screen::AppHeaderContent<'a> {
    crate::screen::AppHeaderContent {
        search: Some(view::build_search_block(&state.search)),
        screen_actions: vec![
            view::build_sort_segment(state.sort),
            view::build_size_segment(state.capsule_size),
            view::build_rescan_button(),
        ],
        second_row: None,
    }
}

use iced::Task;

use crate::app_context::AppContext;
use crate::capsule_cache::CapsuleSize;
use crate::messaging::FooterStatus;
use crate::progress_scan::ProgressData;
use types::{
    CapsuleAsset, GameEntry, ProfileEvent, ProfileViewMessage, ProfileViewPhase, ProfileViewState,
    StoredCapsule,
};

const MAX_CONCURRENT_DOWNLOADS: usize = 2;

pub fn update(
    state: &mut ProfileViewState,
    message: ProfileViewMessage,
    ctx: &mut AppContext,
) -> (Task<ProfileViewMessage>, ProfileEvent) {
    match message {
        ProfileViewMessage::ScanComplete(enumerated) => {
            state.games = enumerated
                .iter()
                .map(|g| GameEntry {
                    app_id: g.app_id,
                    change_number: g.change_number,
                    last_played: g.last_played,
                    name: None,
                    capsule: CapsuleAsset::Pending,
                    progress: None,
                })
                .collect();
            state.capsule_handles.clear();
            state.progress_scanner = None;
            state.progress_rx = None;
            state.phase = ProfileViewPhase::Loaded;

            let app_ids: Vec<u32> = enumerated.iter().map(|g| g.app_id).collect();
            (
                spawn_capsule_queue(app_ids, state.capsule_size),
                ProfileEvent::None,
            )
        }

        ProfileViewMessage::ScanFailed(_) => (Task::none(), ProfileEvent::None),

        ProfileViewMessage::SearchChanged(query) => {
            let q = query.clone();
            let _ = ctx.update_settings(|s| s.library.search = q);
            state.search = query;
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::SortChanged(sort) => {
            let _ = ctx.update_settings(|s| s.library.sort = sort);
            state.sort = sort;
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::CapsuleSizeChanged(new_size) => {
            state.capsule_size = new_size;

            let mut miss_ids: Vec<u32> = Vec::new();
            for entry in &mut state.games {
                let key = (entry.app_id, new_size);
                if let Some(cached) = state.capsule_handles.get(&key) {
                    entry.capsule = CapsuleAsset::Loaded {
                        handle: cached.handle.clone(),
                        width: cached.width,
                        height: cached.height,
                    };
                } else {
                    entry.capsule = CapsuleAsset::Pending;
                    miss_ids.push(entry.app_id);
                }
            }

            let task = if miss_ids.is_empty() {
                Task::none()
            } else {
                spawn_capsule_queue(miss_ids, new_size)
            };
            (task, ProfileEvent::None)
        }

        ProfileViewMessage::CapsuleLoaded {
            app_id,
            size,
            handle,
            width,
            height,
        } => {
            state.capsule_handles.insert(
                (app_id, size),
                StoredCapsule {
                    handle: handle.clone(),
                    width,
                    height,
                },
            );

            if size != state.capsule_size {
                return (Task::none(), ProfileEvent::None);
            }
            if let Some(entry) = state.games.iter_mut().find(|g| g.app_id == app_id) {
                entry.capsule = CapsuleAsset::Loaded {
                    handle,
                    width,
                    height,
                };
            }
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::CapsuleFailed { app_id, size } => {
            if size != state.capsule_size {
                return (Task::none(), ProfileEvent::None);
            }
            if let Some(entry) = state.games.iter_mut().find(|g| g.app_id == app_id) {
                entry.capsule = CapsuleAsset::Unavailable;
            }
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::ProgressFetched {
            app_id,
            earned,
            total,
        } => {
            if let Some(entry) = state.games.iter_mut().find(|g| g.app_id == app_id) {
                entry.progress = Some(ProgressData { earned, total });
            }
            let with_prog = state.games.iter().filter(|g| g.progress.is_some()).count();
            let total_games = state.games.len();
            ctx.messaging.footer = FooterStatus::Scanning {
                current: with_prog,
                total: total_games,
                label: "Loading achievements\u{2026}".to_owned(),
            };
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::ProgressScanDone => {
            ctx.messaging.footer = FooterStatus::Connected {
                games: state.games.len(),
                last_sync: Some(std::time::Instant::now()),
            };
            state.progress_scanner = None;
            state.progress_rx = None;
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::GameSelected(app_id) => {
            if app_id == 0 {
                return (Task::none(), ProfileEvent::None);
            }
            (Task::none(), ProfileEvent::OpenGame(app_id))
        }

        ProfileViewMessage::RescanRequested => {
            state.phase = ProfileViewPhase::Scanning;
            state.games.clear();
            state.capsule_handles.clear();
            state.progress_scanner = None;
            state.progress_rx = None;
            state.loader_pulse_phase = 0.0;
            state.loader_hiding_since = None;
            (Task::none(), ProfileEvent::RequestRescan)
        }

        ProfileViewMessage::SpinnerTick(_) => {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::LoaderPulseTick => {
            use std::time::Instant;

            state.loader_pulse_phase = (state.loader_pulse_phase + 0.04) % 1.0;

            let steam_running = ctx.steam_running;
            if let types::LoaderPhase::Gamma = state.loader_phase(steam_running) {
                if state.loader_hiding_since.is_none() {
                    state.loader_hiding_since = Some(Instant::now());
                }
            } else {
                state.loader_hiding_since = None;
            }
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::CardHoverEnter(app_id) => {
            state.hovered_card = Some(app_id);
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::CardHoverExit(app_id) => {
            if state.hovered_card == Some(app_id) {
                state.hovered_card = None;
            }
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::RetryFailedScans => {
            let ids: Vec<u32> = state.failed_app_ids.iter().copied().collect();
            state.failed_app_ids.clear();
            if !ids.is_empty() {
                let mut scanner = crate::progress_scan::ProgressScanner::new(ids);
                state.progress_rx = scanner.take_receiver();
                state.progress_scanner = Some(scanner);
            }
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::BarSliceHoverEnter(tier) => {
            state.hovered_bar_slice = Some(tier);
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::BarSliceHoverExit => {
            state.hovered_bar_slice = None;
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::CardTierHovered { app_id, tier } => {
            state.hovered_card_tier = tier.map(|t| (app_id, t));
            (Task::none(), ProfileEvent::None)
        }

        ProfileViewMessage::RequestToggleGamePin(id) => {
            (Task::none(), ProfileEvent::ToggleGamePin(id))
        }

        ProfileViewMessage::RequestOpenGame(id) => (Task::none(), ProfileEvent::OpenGame(id)),

        ProfileViewMessage::DrainProgressResults => drain_progress_results(state, ctx),
    }
}

fn drain_progress_results(
    state: &mut ProfileViewState,
    ctx: &mut AppContext,
) -> (Task<ProfileViewMessage>, ProfileEvent) {
    if let Some(scanner) = &mut state.progress_scanner {
        let _still_going = scanner.poll();
    }

    let Some(rx) = &mut state.progress_rx else {
        return (Task::none(), ProfileEvent::None);
    };

    let mut cache_entries: Vec<crate::cache::GameCacheEntry> = Vec::new();
    let mut no_ach_events: Vec<(u32, u32)> = Vec::new();
    let mut tasks: Vec<Task<ProfileViewMessage>> = Vec::new();

    loop {
        match rx.try_recv() {
            Ok(result) => {
                let scan_app_id = result.app_id;
                let Some(data) = result.data else {
                    state.failed_app_ids.insert(scan_app_id);
                    tasks.push(Task::done(ProfileViewMessage::ScanFailed(format!(
                        "Scan failed for app {scan_app_id}"
                    ))));
                    continue;
                };

                if data.achievements.is_empty() {
                    let change_number = state
                        .games
                        .iter()
                        .find(|g| g.app_id == scan_app_id)
                        .map(|g| g.change_number);
                    state.games.retain(|g| g.app_id != scan_app_id);
                    ctx.cached_entries.remove(&scan_app_id);
                    if let Some(cn) = change_number {
                        ctx.no_ach_cache.insert(scan_app_id, cn);
                        no_ach_events.push((scan_app_id, cn));
                    }
                    continue;
                }

                let earned = data.earned_count();
                let total = data.total_count();
                tasks.push(Task::done(ProfileViewMessage::ProgressFetched {
                    app_id: scan_app_id,
                    earned,
                    total,
                }));

                if let Some(scanned_name) = &data.app_name
                    && let Some(game) = state.games.iter_mut().find(|g| g.app_id == scan_app_id)
                {
                    game.name = Some(scanned_name.clone());
                }

                if let Some(game) = state.games.iter().find(|g| g.app_id == scan_app_id) {
                    let entry = crate::build_cache_entry_from_scan(
                        &data,
                        scan_app_id,
                        game.name.as_deref(),
                        &ctx.steam_root,
                        ctx.steamid3,
                    );
                    ctx.cached_entries.insert(scan_app_id, entry.clone());
                    cache_entries.push(entry);
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                tasks.push(Task::done(ProfileViewMessage::ProgressScanDone));
                break;
            }
        }
    }

    let task = if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    };

    let event = if cache_entries.is_empty() && no_ach_events.is_empty() {
        ProfileEvent::None
    } else {
        ProfileEvent::DrainedProgress {
            cache_entries,
            no_ach_entries: no_ach_events,
        }
    };

    (task, event)
}

fn spawn_capsule_queue(app_ids: Vec<u32>, size: CapsuleSize) -> Task<ProfileViewMessage> {
    let chunks: Vec<Vec<u32>> = app_ids
        .chunks(MAX_CONCURRENT_DOWNLOADS)
        .map(|c| c.to_vec())
        .collect();

    let tasks: Vec<Task<ProfileViewMessage>> = chunks
        .into_iter()
        .map(|chunk| {
            let batch: Vec<Task<ProfileViewMessage>> = chunk
                .into_iter()
                .map(|app_id| crate::capsule_commands::fetch_capsule(app_id, size))
                .collect();
            Task::batch(batch)
        })
        .collect();

    Task::batch(tasks)
}

pub fn subscription(
    state: &ProfileViewState,
    steam_running: Option<bool>,
) -> iced::Subscription<ProfileViewMessage> {
    use iced::time;

    let spinner_sub = if state.is_streaming() {
        time::every(std::time::Duration::from_millis(80))
            .map(|_| ProfileViewMessage::SpinnerTick(0.0))
    } else {
        iced::Subscription::none()
    };

    let loader_pulse_sub = if state.loader_needs_pulse_subscription(steam_running) {
        time::every(std::time::Duration::from_millis(70))
            .map(|_| ProfileViewMessage::LoaderPulseTick)
    } else {
        iced::Subscription::none()
    };

    let progress_drain_sub = if state.progress_scanner.is_some() {
        time::every(std::time::Duration::from_millis(200))
            .map(|_| ProfileViewMessage::DrainProgressResults)
    } else {
        iced::Subscription::none()
    };

    iced::Subscription::batch([spinner_sub, loader_pulse_sub, progress_drain_sub])
}

pub fn render<'a>(
    state: &'a ProfileViewState,
    props: view::ProfileViewProps<'a>,
) -> crate::screen::ScreenContent<'a, ProfileViewMessage> {
    view::render(state, props)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_subscription_constructs_with_default_state() {
        let state = ProfileViewState::new();
        let _: iced::Subscription<ProfileViewMessage> = subscription(&state, None);
    }

    #[test]
    fn profile_subscription_constructs_steam_running() {
        let state = ProfileViewState::new();
        let _: iced::Subscription<ProfileViewMessage> = subscription(&state, Some(true));
    }
}
