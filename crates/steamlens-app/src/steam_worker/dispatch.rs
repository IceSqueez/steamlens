use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc;

use steamlens_core::AchievementIcon;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorKind, WorkerResponse};

use super::api::SteamReply;
use super::apply::run_apply_sequence;
use crate::timeouts;
use crate::worker_subprocess::WorkerHandle;

pub(super) fn reply(tx: &mpsc::UnboundedSender<SteamReply>, r: SteamReply) {
    let _ = tx.send(r);
}

pub(super) fn error_reply(kind: WorkerErrorKind, message: String) -> SteamReply {
    match kind {
        WorkerErrorKind::Connect | WorkerErrorKind::NotLoggedIn => {
            SteamReply::ConnectFailed(message)
        }
        WorkerErrorKind::RequestUserStats
        | WorkerErrorKind::UserStatsReceived
        | WorkerErrorKind::PollCallbacks => SteamReply::LoadFailed(message),
        WorkerErrorKind::StoreStats | WorkerErrorKind::UserStatsStored => {
            SteamReply::SaveFailed(message)
        }
        WorkerErrorKind::RequestGlobalPercentages
        | WorkerErrorKind::GlobalPercentagesReady
        | WorkerErrorKind::GlobalPercentagesAPICall => SteamReply::GlobalPercentagesFailed,
        WorkerErrorKind::Generic => SteamReply::LoadFailed(message),
    }
}

fn is_unsolicited(resp: &WorkerResponse) -> bool {
    matches!(
        resp,
        WorkerResponse::IconUpdated { .. } | WorkerResponse::GlobalPercentagesReady { .. }
    )
}

pub(super) async fn round_trip(
    handle: &mut WorkerHandle,
    cmd: &WorkerCommand,
    timeout: Duration,
    rep_tx: &mpsc::UnboundedSender<SteamReply>,
) -> Option<WorkerResponse> {
    if handle.send(cmd).await.is_err() {
        return None;
    }
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, handle.recv()).await {
            Ok(Ok(Some(resp))) => {
                if is_unsolicited(&resp) {
                    handle_worker_response(resp, rep_tx);
                    continue;
                }
                return Some(resp);
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
    resp: WorkerResponse,
    rep_tx: &mpsc::UnboundedSender<SteamReply>,
) {
    match resp {
        WorkerResponse::SteamConnected { app_name, .. } => {
            reply(rep_tx, SteamReply::Connected { app_name });
        }
        WorkerResponse::Ack => {}
        WorkerResponse::AchievementsAndStats {
            shm_path,
            region_bytes,
        } => match read_shm::<steamlens_core::AchievementsAndStatsPayload>(
            "AchievementsAndStats",
            &shm_path,
            region_bytes,
        ) {
            Ok(p) => reply(
                rep_tx,
                SteamReply::AchievementsAndStats {
                    achievements: p.achievements,
                    stats: p.stats,
                },
            ),
            Err(msg) => reply(rep_tx, SteamReply::LoadFailed(msg)),
        },
        WorkerResponse::IconUpdated {
            name,
            shm_path,
            region_bytes,
        } => match read_shm::<AchievementIcon>("IconUpdated", &shm_path, region_bytes) {
            Ok(icon) => reply(rep_tx, SteamReply::IconUpdated { name, icon }),
            Err(msg) => {
                tracing::error!("icon shm read failed for {name}: {msg}");
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
            Ok(map) => reply(rep_tx, SteamReply::GlobalPercentagesReady(map)),
            Err(_) => reply(rep_tx, SteamReply::GlobalPercentagesFailed),
        },
        WorkerResponse::Stored => {
            reply(rep_tx, SteamReply::ChangesSaved);
        }
        WorkerResponse::AchievementCount {
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
        WorkerResponse::CardOnlyAchievements {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&PathBuf::from(shm_path));
        }
        WorkerResponse::Error { kind, message } => {
            reply(rep_tx, error_reply(kind, message));
        }
        WorkerResponse::Disconnected => {
            reply(rep_tx, SteamReply::Disconnected);
        }
    }
}

pub(super) async fn handle_request(
    req: super::api::SteamRequest,
    handle: &mut WorkerHandle,
    rep_tx: &mpsc::UnboundedSender<SteamReply>,
) {
    use super::api::SteamRequest;
    match req {
        SteamRequest::RequestUserStats => {
            let timeout = timeouts::LIVE_LOAD;
            match round_trip(
                handle,
                &WorkerCommand::LoadAchievementsAndStats,
                timeout,
                rep_tx,
            )
            .await
            {
                Some(resp) => handle_worker_response(resp, rep_tx),
                None => reply(
                    rep_tx,
                    SteamReply::LoadFailed("timed out waiting for AchievementsAndStats".to_owned()),
                ),
            }
        }

        SteamRequest::RequestGlobalPercentages => {
            let timeout = timeouts::GLOBAL_PERCENTAGES;
            match round_trip(
                handle,
                &WorkerCommand::RequestGlobalPercentages,
                timeout,
                rep_tx,
            )
            .await
            {
                Some(resp) => handle_worker_response(resp, rep_tx),
                None => reply(rep_tx, SteamReply::GlobalPercentagesFailed),
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
                rep_tx,
            )
            .await;
        }

        SteamRequest::ConnectWithApp(_) | SteamRequest::Disconnect => {}
    }
}
