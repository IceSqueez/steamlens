pub mod profile;
pub mod types;
mod view;

use iced::Task;
use iced::widget::image::Handle as ImageHandle;

use crate::capsule_cache::{self, CapsuleSize};
use crate::progress_scan::ProgressData;
use crate::steam_worker::{SteamRequest, SteamWorker};

use types::{
    CapsuleState, FADE_DELTA, GameEntry, ProfileViewMessage, ProfileViewPhase, ProfileViewState,
    StoredCapsule,
};

const MAX_CONCURRENT_DOWNLOADS: usize = 2;

pub fn update(state: &mut ProfileViewState, message: ProfileViewMessage) -> Task<crate::Message> {
    match message {
        ProfileViewMessage::ScanComplete(summaries) => {
            state.games = summaries
                .iter()
                .map(|s| GameEntry {
                    summary: s.clone(),
                    capsule: CapsuleState::Pending,
                    revealed: false,
                    progress: None,
                })
                .collect();
            state.reveal_queue.clear();
            state.capsule_handles.clear();
            state.progress_scanner = None;
            state.progress_rx = None;
            state.phase = ProfileViewPhase::Loaded;

            let app_ids: Vec<u32> = summaries.iter().map(|s| s.app_id).collect();
            spawn_capsule_queue(app_ids, state.capsule_size)
        }

        ProfileViewMessage::ScanFailed(reason) => {
            state.phase = ProfileViewPhase::Error(reason);
            Task::none()
        }

        ProfileViewMessage::SearchChanged(query) => {
            state.search = query;
            Task::none()
        }

        ProfileViewMessage::SortChanged(sort) => {
            state.sort = sort;
            Task::none()
        }

        ProfileViewMessage::CapsuleSizeChanged(new_size) => {
            state.capsule_size = new_size;
            state.reveal_queue.clear();

            let mut miss_ids: Vec<u32> = Vec::new();
            for entry in &mut state.games {
                let key = (entry.summary.app_id, new_size);
                if let Some(cached) = state.capsule_handles.get(&key) {
                    entry.capsule = CapsuleState::Loaded {
                        handle: cached.handle.clone(),
                        width: cached.width,
                        height: cached.height,
                        opacity: 1.0,
                    };
                    entry.revealed = true;
                } else {
                    entry.capsule = CapsuleState::Pending;
                    entry.revealed = false;
                    miss_ids.push(entry.summary.app_id);
                }
            }

            if miss_ids.is_empty() {
                Task::none()
            } else {
                spawn_capsule_queue(miss_ids, new_size)
            }
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
                return Task::none();
            }
            if let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id) {
                entry.capsule = CapsuleState::Loaded {
                    handle,
                    width,
                    height,
                    opacity: 0.0,
                };
            }
            state.reveal_queue.push_back(app_id);
            Task::none()
        }

        ProfileViewMessage::RevealTick => {
            let pop_count = state.reveal_queue.len().min(3);
            for _ in 0..pop_count {
                if let Some(app_id) = state.reveal_queue.pop_front()
                    && let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id)
                {
                    entry.revealed = true;
                }
            }
            Task::none()
        }

        ProfileViewMessage::FadeTick => {
            for entry in &mut state.games {
                if let CapsuleState::Loaded { opacity, .. } = &mut entry.capsule
                    && *opacity < 1.0
                {
                    *opacity = (*opacity + FADE_DELTA).min(1.0);
                }
            }
            Task::none()
        }

        ProfileViewMessage::CapsuleFailed { app_id, size } => {
            if size != state.capsule_size {
                return Task::none();
            }
            if let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id) {
                entry.capsule = CapsuleState::Unavailable;
            }
            Task::none()
        }

        ProfileViewMessage::ProgressFetched {
            app_id,
            earned,
            total,
        } => {
            if let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id) {
                entry.progress = Some(ProgressData { earned, total });
            }
            Task::none()
        }

        ProfileViewMessage::ProgressScanDone => {
            state.progress_scanner = None;
            state.progress_rx = None;
            Task::none()
        }

        ProfileViewMessage::GameSelected(_) => Task::none(),

        ProfileViewMessage::ManualAppIdChanged(s) => {
            state.manual_app_id_input = s.chars().filter(|c| c.is_ascii_digit()).collect();
            Task::none()
        }

        ProfileViewMessage::ManualAppIdSubmitted => Task::none(),

        ProfileViewMessage::RescanRequested => {
            state.phase = ProfileViewPhase::Scanning;
            state.games.clear();
            state.reveal_queue.clear();
            state.capsule_handles.clear();
            state.progress_scanner = None;
            state.progress_rx = None;
            state.loader_pulse_phase = 0.0;
            state.loader_hiding_since = None;
            Task::none()
        }

        ProfileViewMessage::SpinnerTick(_) => {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
            Task::none()
        }

        ProfileViewMessage::LoaderPulseTick => {
            use std::time::Instant;

            state.loader_pulse_phase = (state.loader_pulse_phase + 0.04) % 1.0;

            if let types::LoaderPhase::Gamma = state.loader_phase() {
                if state.loader_hiding_since.is_none() {
                    state.loader_hiding_since = Some(Instant::now());
                }
            } else {
                state.loader_hiding_since = None;
            }
            Task::none()
        }
    }
}

fn spawn_capsule_queue(app_ids: Vec<u32>, size: CapsuleSize) -> Task<crate::Message> {
    let chunks: Vec<Vec<u32>> = app_ids
        .chunks(MAX_CONCURRENT_DOWNLOADS)
        .map(|c| c.to_vec())
        .collect();

    let tasks: Vec<Task<crate::Message>> = chunks
        .into_iter()
        .map(|chunk| {
            let batch: Vec<Task<crate::Message>> = chunk
                .into_iter()
                .map(|app_id| {
                    Task::perform(
                        async move { capsule_cache::fetch_capsule(app_id, size).await },
                        move |result| match result {
                            Ok((fetched_size, pixels)) => {
                                let handle = ImageHandle::from_rgba(
                                    pixels.width,
                                    pixels.height,
                                    pixels.rgba,
                                );
                                crate::Message::ProfileView(ProfileViewMessage::CapsuleLoaded {
                                    app_id,
                                    size: fetched_size,
                                    handle,
                                    width: pixels.width,
                                    height: pixels.height,
                                })
                            }
                            Err((fetched_size, _)) => {
                                crate::Message::ProfileView(ProfileViewMessage::CapsuleFailed {
                                    app_id,
                                    size: fetched_size,
                                })
                            }
                        },
                    )
                })
                .collect();
            Task::batch(batch)
        })
        .collect();

    Task::batch(tasks)
}

pub fn trigger_scan(worker: &SteamWorker) {
    worker.send(SteamRequest::ScanLibrary);
}

pub fn view_with_cache_actions<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a std::collections::HashMap<u32, crate::cache::GameCacheEntry>,
) -> iced::Element<'a, crate::Message> {
    view::render_with_cache_actions(state, user_profile, cached_entries)
}
