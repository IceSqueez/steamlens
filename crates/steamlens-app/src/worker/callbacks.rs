use std::time::Instant;

use steamlens_core::ipc::{WorkerErrorKind, WorkerResponse};
use steamlens_core::{Client, SteamCallback};

use super::ipc_io::write_response;
use super::shm_responses::{build_icon_response, shm_response_for_aas, shm_response_for_card_only};
use crate::timeouts;

pub(super) async fn wait_for_stats_received(
    client: &Client,
    expected_user: u64,
) -> Option<WorkerResponse> {
    let deadline = Instant::now() + timeouts::STAT_RECEIVED;
    loop {
        if let Ok(callbacks) = client.poll_callbacks() {
            for cb in &callbacks {
                if let SteamCallback::UserStatsReceived {
                    result,
                    user_steam_id,
                    game_id,
                    ..
                } = cb
                {
                    if *user_steam_id != expected_user {
                        continue;
                    }
                    tracing::debug!(
                        "UserStatsReceived: result={} game={}",
                        result.raw(),
                        game_id,
                    );
                    if result.is_ok() {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return None;
                    }
                    if result.raw() == steamlens_core::STEAM_RESULT_NO_STATS_SCHEMA {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return Some(shm_response_for_aas(
                            steamlens_core::AchievementsAndStatsPayload {
                                achievements: Vec::new(),
                                stats: Vec::new(),
                                genre: None,
                            },
                        ));
                    }
                    forward_icon_callbacks(callbacks.clone(), client).await;
                    return Some(WorkerResponse::Error {
                        kind: WorkerErrorKind::UserStatsReceived,
                        message: format!("result code {}", result.raw()),
                    });
                }
            }
            forward_icon_callbacks(callbacks, client).await;
        }

        if Instant::now() >= deadline {
            return Some(WorkerResponse::Error {
                kind: WorkerErrorKind::UserStatsReceived,
                message: "timed out waiting for UserStatsReceived".into(),
            });
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

pub(super) async fn wait_for_stats_received_card_only(
    client: &Client,
    expected_user: u64,
) -> Option<WorkerResponse> {
    let deadline = Instant::now() + timeouts::STAT_RECEIVED;
    loop {
        if let Ok(callbacks) = client.poll_callbacks() {
            for cb in &callbacks {
                if let SteamCallback::UserStatsReceived {
                    result,
                    user_steam_id,
                    game_id,
                    ..
                } = cb
                {
                    if *user_steam_id != expected_user {
                        continue;
                    }
                    tracing::debug!(
                        "UserStatsReceived (card-only): result={} game={}",
                        result.raw(),
                        game_id,
                    );
                    if result.is_ok() {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return None;
                    }
                    if result.raw() == steamlens_core::STEAM_RESULT_NO_STATS_SCHEMA {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return Some(shm_response_for_card_only(
                            steamlens_core::CardOnlyPayload {
                                achievements: Vec::new(),
                                genre: None,
                            },
                        ));
                    }
                    forward_icon_callbacks(callbacks.clone(), client).await;
                    return Some(WorkerResponse::Error {
                        kind: WorkerErrorKind::UserStatsReceived,
                        message: format!("result code {}", result.raw()),
                    });
                }
            }
            forward_icon_callbacks(callbacks, client).await;
        }

        if Instant::now() >= deadline {
            tracing::warn!(
                "wait_for_stats_received_card_only TIMEOUT after {:?} (no callback fired)",
                timeouts::STAT_RECEIVED
            );
            return Some(WorkerResponse::Error {
                kind: WorkerErrorKind::UserStatsReceived,
                message: "timed out waiting for UserStatsReceived".into(),
            });
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

pub(super) async fn wait_for_store_confirmed(client: &Client) -> WorkerResponse {
    let deadline = Instant::now() + timeouts::STORE_CONFIRMED;
    loop {
        if let Ok(callbacks) = client.poll_callbacks() {
            for cb in &callbacks {
                if let SteamCallback::UserStatsStored { result, .. } = cb {
                    if result.is_ok() {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return WorkerResponse::Stored;
                    } else {
                        forward_icon_callbacks(callbacks.clone(), client).await;
                        return WorkerResponse::Error {
                            kind: WorkerErrorKind::UserStatsStored,
                            message: format!("result code {}", result.raw()),
                        };
                    }
                }
            }
            forward_icon_callbacks(callbacks, client).await;
        }

        if Instant::now() >= deadline {
            return WorkerResponse::Error {
                kind: WorkerErrorKind::StoreStats,
                message: "timed out waiting for UserStatsStored".into(),
            };
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

pub(super) async fn forward_icon_callbacks(callbacks: Vec<SteamCallback>, client: &Client) {
    let mut icon_count = 0usize;
    for cb in callbacks {
        if let SteamCallback::UserAchievementIconFetched {
            achievement_name,
            icon_handle,
            ..
        } = cb
        {
            if icon_handle == 0 {
                tracing::trace!(name = %achievement_name, "forward_icon_callbacks: skip handle=0");
                continue;
            }
            match client.get_image(icon_handle) {
                Ok(Some(img)) => {
                    tracing::trace!(name = %achievement_name, handle = icon_handle, w = img.width, h = img.height, "forward_icon_callbacks: fetched");
                    let resp = build_icon_response(achievement_name, img);
                    let _ = write_response(&resp).await;
                    icon_count += 1;
                }
                Ok(None) => {
                    tracing::trace!(name = %achievement_name, handle = icon_handle, "forward_icon_callbacks: get_image returned None");
                }
                Err(e) => {
                    tracing::trace!(name = %achievement_name, handle = icon_handle, error = %e, "forward_icon_callbacks: get_image failed");
                }
            }
        }
    }
    if icon_count > 0 {
        tracing::trace!(icon_count, "forward_icon_callbacks: batch sent");
    }
}

pub(super) async fn poll_and_forward(client: &Client) {
    if let Ok(callbacks) = client.poll_callbacks() {
        forward_icon_callbacks(callbacks, client).await;
    }
}
