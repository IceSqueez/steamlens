pub mod types;
mod view;

use iced::Task;
use iced::widget::image::Handle as ImageHandle;

use crate::capsule_cache;
use crate::steam_worker::{SteamReply, SteamRequest, SteamWorker};

use types::{CapsuleState, GameEntry, LibraryMessage, LibraryPhase, LibraryState};

const MAX_CONCURRENT_DOWNLOADS: usize = 8;

pub fn handle_steam_reply(state: &mut LibraryState, reply: SteamReply) -> Task<crate::Message> {
    match reply {
        SteamReply::LibraryScan(games) => {
            let msg = LibraryMessage::ScanComplete(games);
            update(state, msg)
        }
        SteamReply::LibraryScanFailed(e) => {
            let msg = LibraryMessage::ScanFailed(e);
            update(state, msg)
        }
        _ => Task::none(),
    }
}

pub fn update(state: &mut LibraryState, message: LibraryMessage) -> Task<crate::Message> {
    match message {
        LibraryMessage::ScanComplete(summaries) => {
            state.games = summaries
                .iter()
                .map(|s| GameEntry {
                    summary: s.clone(),
                    capsule: CapsuleState::Pending,
                })
                .collect();
            state.phase = LibraryPhase::Loaded;

            let app_ids: Vec<u32> = summaries.iter().map(|s| s.app_id).collect();
            spawn_capsule_queue(app_ids)
        }

        LibraryMessage::ScanFailed(reason) => {
            state.phase = LibraryPhase::Error(reason);
            Task::none()
        }

        LibraryMessage::SearchChanged(query) => {
            state.search = query;
            Task::none()
        }

        LibraryMessage::SortChanged(sort) => {
            state.sort = sort;
            Task::none()
        }

        LibraryMessage::CardWidthChanged(w) => {
            state.card_width = w;
            Task::none()
        }

        LibraryMessage::CapsuleLoaded {
            app_id,
            handle,
            width,
            height,
        } => {
            if let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id) {
                entry.capsule = CapsuleState::Loaded {
                    handle,
                    width,
                    height,
                };
            }
            Task::none()
        }

        LibraryMessage::CapsuleFailed(app_id) => {
            if let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id) {
                entry.capsule = CapsuleState::Unavailable;
            }
            Task::none()
        }

        LibraryMessage::GameSelected(_) => Task::none(),

        LibraryMessage::ManualAppIdChanged(s) => {
            state.manual_app_id_input = s.chars().filter(|c| c.is_ascii_digit()).collect();
            Task::none()
        }

        LibraryMessage::ManualAppIdSubmitted => Task::none(),

        LibraryMessage::RescanRequested => {
            state.phase = LibraryPhase::Scanning;
            state.games.clear();
            Task::none()
        }
    }
}

fn spawn_capsule_queue(app_ids: Vec<u32>) -> Task<crate::Message> {
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
                        async move { capsule_cache::fetch_capsule(app_id).await },
                        move |result| match result {
                            Ok(pixels) => {
                                let handle = ImageHandle::from_rgba(
                                    pixels.width,
                                    pixels.height,
                                    pixels.rgba,
                                );
                                crate::Message::Library(LibraryMessage::CapsuleLoaded {
                                    app_id,
                                    handle,
                                    width: pixels.width,
                                    height: pixels.height,
                                })
                            }
                            Err(_) => {
                                crate::Message::Library(LibraryMessage::CapsuleFailed(app_id))
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

pub fn view(state: &LibraryState) -> iced::Element<'_, crate::Message> {
    view::render(state)
}
