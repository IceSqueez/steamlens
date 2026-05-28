use std::collections::HashMap;
use std::sync::mpsc;

use steamlens_core::ipc::{WorkerCommand, WorkerResponse};

use super::api::SteamReply;
use super::dispatch::{error_reply, handle_worker_response, reply, round_trip};
use crate::timeouts;
use crate::worker_subprocess::WorkerHandle;

pub(super) async fn run_apply_sequence(
    achievements_to_set: Vec<String>,
    achievements_to_clear: Vec<String>,
    stats_int: HashMap<String, i32>,
    stats_float: HashMap<String, f32>,
    handle: &mut WorkerHandle,
    rep_tx: &mpsc::Sender<SteamReply>,
) {
    let staging_timeout = timeouts::STAGING;

    let mut staging_cmds = Vec::new();
    for name in achievements_to_set {
        staging_cmds.push(WorkerCommand::SetAchievement(name));
    }
    for name in achievements_to_clear {
        staging_cmds.push(WorkerCommand::ClearAchievement(name));
    }
    for (name, value) in stats_int {
        staging_cmds.push(WorkerCommand::SetStatInt { name, value });
    }
    for (name, value) in stats_float {
        staging_cmds.push(WorkerCommand::SetStatFloat { name, value });
    }

    for cmd in &staging_cmds {
        match round_trip(handle, cmd, staging_timeout, rep_tx).await {
            Some(WorkerResponse::Ack) => {}
            Some(WorkerResponse::Error { kind, message }) => {
                reply(rep_tx, error_reply(kind, message));
                return;
            }
            Some(other) => {
                reply(
                    rep_tx,
                    SteamReply::SaveFailed(format!(
                        "unexpected response during staging: {:?}",
                        std::mem::discriminant(&other)
                    )),
                );
                return;
            }
            None => {
                reply(
                    rep_tx,
                    SteamReply::SaveFailed("timed out waiting for staging Ack".to_owned()),
                );
                return;
            }
        }
    }

    let store_timeout = timeouts::LIVE_LOAD;
    match round_trip(handle, &WorkerCommand::StoreStats, store_timeout, rep_tx).await {
        Some(WorkerResponse::Stored) => {
            reply(rep_tx, SteamReply::ChangesSaved);
            let load_timeout = timeouts::LIVE_LOAD;
            match round_trip(
                handle,
                &WorkerCommand::LoadAchievementsAndStats,
                load_timeout,
                rep_tx,
            )
            .await
            {
                Some(resp) => handle_worker_response(resp, rep_tx),
                None => reply(
                    rep_tx,
                    SteamReply::LoadFailed("timed out after store".into()),
                ),
            }
        }
        Some(WorkerResponse::Error { kind, message }) => {
            reply(rep_tx, error_reply(kind, message));
        }
        Some(other) => {
            reply(
                rep_tx,
                SteamReply::SaveFailed(format!(
                    "unexpected StoreStats response: {:?}",
                    std::mem::discriminant(&other)
                )),
            );
        }
        None => {
            reply(
                rep_tx,
                SteamReply::SaveFailed("timed out waiting for StoreStats".to_owned()),
            );
        }
    }
}
