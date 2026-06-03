use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc;

use steamlens_core::AchievementIcon;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorStage, WorkerResponse};

use super::api::{SteamReply, WorkerReply};
use super::apply::run_apply_sequence;
use crate::timeouts;
use crate::worker_subprocess::WorkerHandle;

pub(super) fn reply(sender: &mpsc::UnboundedSender<WorkerReply>, app_id: u32, message: SteamReply) {
    let _ = sender.send(WorkerReply {
        app_id,
        reply: message,
    });
}

pub(super) fn error_reply(stage: WorkerErrorStage, message: String) -> SteamReply {
    match stage {
        WorkerErrorStage::Connect | WorkerErrorStage::NotLoggedIn => {
            SteamReply::ConnectFailed(message)
        }
        WorkerErrorStage::RequestUserStats
        | WorkerErrorStage::UserStatsReceived
        | WorkerErrorStage::PollCallbacks => SteamReply::LoadFailed(message),
        WorkerErrorStage::StoreStats | WorkerErrorStage::UserStatsStored => {
            SteamReply::SaveFailed(message)
        }
        WorkerErrorStage::RequestGlobalPercentages
        | WorkerErrorStage::GlobalPercentagesReady
        | WorkerErrorStage::GlobalPercentagesApiCall => SteamReply::GlobalPercentagesFailed,
        WorkerErrorStage::Generic => SteamReply::LoadFailed(message),
    }
}

fn is_unsolicited(response: &WorkerResponse) -> bool {
    matches!(
        response,
        WorkerResponse::IconUpdated { .. } | WorkerResponse::GlobalPercentagesReady { .. }
    )
}

pub(super) async fn round_trip(
    handle: &mut WorkerHandle,
    command: &WorkerCommand,
    timeout: Duration,
    reply_sender: &mpsc::UnboundedSender<WorkerReply>,
    app_id: u32,
) -> Option<WorkerResponse> {
    if handle.send(command).await.is_err() {
        return None;
    }
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, handle.recv()).await {
            Ok(Ok(Some(response))) => {
                if is_unsolicited(&response) {
                    handle_worker_response(response, reply_sender, app_id);
                    continue;
                }
                return Some(response);
            }
            _ => return None,
        }
    }
}

fn read_shm<T: serde::de::DeserializeOwned>(
    label: &str,
    shm_path: &str,
    region_bytes: u64,
) -> Result<T, String> {
    let path = PathBuf::from(shm_path);
    steamlens_core::read_payload::<T>(&path, region_bytes)
        .map_err(|e| format!("{label} shm read at {}: {e}", path.display()))
}

pub(super) fn handle_worker_response(
    response: WorkerResponse,
    reply_sender: &mpsc::UnboundedSender<WorkerReply>,
    app_id: u32,
) {
    match response {
        WorkerResponse::SteamConnected { app_name, .. } => {
            reply(reply_sender, app_id, SteamReply::Connected { app_name });
        }
        WorkerResponse::Ack => {}
        WorkerResponse::AchievementsFull {
            shm_path,
            region_bytes,
        } => match read_shm::<steamlens_core::AchievementsFullPayload>(
            "AchievementsFull",
            &shm_path,
            region_bytes,
        ) {
            Ok(payload) => reply(
                reply_sender,
                app_id,
                SteamReply::AchievementsFull {
                    achievements: payload.achievements,
                    stats: payload.stats,
                },
            ),
            Err(err_msg) => reply(reply_sender, app_id, SteamReply::LoadFailed(err_msg)),
        },
        WorkerResponse::IconUpdated {
            name,
            shm_path,
            region_bytes,
        } => match read_shm::<AchievementIcon>("IconUpdated", &shm_path, region_bytes) {
            Ok(icon) => reply(reply_sender, app_id, SteamReply::IconUpdated { name, icon }),
            Err(err_msg) => {
                tracing::error!("icon shm read failed for {name}: {err_msg}");
            }
        },
        WorkerResponse::GlobalPercentagesReady {
            shm_path,
            region_bytes,
        } => match read_shm::<HashMap<String, f32>>(
            "GlobalPercentagesReady",
            &shm_path,
            region_bytes,
        ) {
            Ok(map) => reply(
                reply_sender,
                app_id,
                SteamReply::GlobalPercentagesReady(map),
            ),
            Err(_) => reply(reply_sender, app_id, SteamReply::GlobalPercentagesFailed),
        },
        WorkerResponse::StatsStored => {
            reply(reply_sender, app_id, SteamReply::ChangesSaved);
        }
        WorkerResponse::AchievementsCount {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&PathBuf::from(shm_path));
        }
        WorkerResponse::ProbeResult {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&PathBuf::from(shm_path));
        }
        WorkerResponse::AchievementsSummary {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&PathBuf::from(shm_path));
        }
        WorkerResponse::Error { stage, message } => {
            reply(reply_sender, app_id, error_reply(stage, message));
        }
        WorkerResponse::Disconnected => {
            reply(reply_sender, app_id, SteamReply::Disconnected);
        }
    }
}

pub(super) async fn handle_request(
    request: super::api::SteamRequest,
    handle: &mut WorkerHandle,
    reply_sender: &mpsc::UnboundedSender<WorkerReply>,
    app_id: u32,
) {
    use super::api::SteamRequest;
    match request {
        SteamRequest::RequestUserStats => {
            let timeout = timeouts::LIVE_LOAD;
            match round_trip(
                handle,
                &WorkerCommand::LoadAchievementsFull,
                timeout,
                reply_sender,
                app_id,
            )
            .await
            {
                Some(response) => handle_worker_response(response, reply_sender, app_id),
                None => reply(
                    reply_sender,
                    app_id,
                    SteamReply::LoadFailed("timed out waiting for AchievementsFull".to_owned()),
                ),
            }
        }

        SteamRequest::RequestGlobalPercentages => {
            let timeout = timeouts::GLOBAL_PERCENTAGES;
            match round_trip(
                handle,
                &WorkerCommand::RequestGlobalPercentages,
                timeout,
                reply_sender,
                app_id,
            )
            .await
            {
                Some(response) => handle_worker_response(response, reply_sender, app_id),
                None => reply(reply_sender, app_id, SteamReply::GlobalPercentagesFailed),
            }
        }

        SteamRequest::ApplyChanges {
            achievements_to_set,
            achievements_to_clear,
            stats_int,
            stats_float,
        } => {
            run_apply_sequence(
                achievements_to_set,
                achievements_to_clear,
                stats_int,
                stats_float,
                handle,
                reply_sender,
                app_id,
            )
            .await;
        }

        SteamRequest::ConnectWithApp(_) => {}
    }
}
