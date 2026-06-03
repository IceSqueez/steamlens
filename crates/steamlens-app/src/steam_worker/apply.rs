use std::collections::HashMap;
use tokio::sync::mpsc;

use steamlens_core::ipc::{WorkerCommand, WorkerResponse};

use super::api::{SteamReply, WorkerReply};
use super::dispatch::{error_reply, handle_worker_response, reply, round_trip};
use crate::timeouts;
use crate::worker_subprocess::WorkerHandle;

pub(super) async fn run_apply_sequence(
    achievements_to_set: Vec<String>,
    achievements_to_clear: Vec<String>,
    stats_int: HashMap<String, i32>,
    stats_float: HashMap<String, f32>,
    handle: &mut WorkerHandle,
    reply_sender: &mpsc::UnboundedSender<WorkerReply>,
    app_id: u32,
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

    for command in &staging_cmds {
        match round_trip(handle, command, staging_timeout, reply_sender, app_id).await {
            Some(WorkerResponse::Ack) => {}
            Some(WorkerResponse::Error { stage, message }) => {
                reply(reply_sender, app_id, error_reply(stage, message));
                return;
            }
            Some(other) => {
                reply(
                    reply_sender,
                    app_id,
                    SteamReply::SaveFailed(format!(
                        "unexpected response during staging: {:?}",
                        std::mem::discriminant(&other)
                    )),
                );
                return;
            }
            None => {
                reply(
                    reply_sender,
                    app_id,
                    SteamReply::SaveFailed("timed out waiting for staging Ack".to_owned()),
                );
                return;
            }
        }
    }

    let store_timeout = timeouts::LIVE_LOAD;
    match round_trip(
        handle,
        &WorkerCommand::StoreStats,
        store_timeout,
        reply_sender,
        app_id,
    )
    .await
    {
        Some(WorkerResponse::StatsStored) => {
            reply(reply_sender, app_id, SteamReply::ChangesSaved);
            let load_timeout = timeouts::LIVE_LOAD;
            match round_trip(
                handle,
                &WorkerCommand::LoadAchievementsFull,
                load_timeout,
                reply_sender,
                app_id,
            )
            .await
            {
                Some(response) => handle_worker_response(response, reply_sender, app_id),
                None => reply(
                    reply_sender,
                    app_id,
                    SteamReply::LoadFailed("timed out after store".into()),
                ),
            }
        }
        Some(WorkerResponse::Error { stage, message }) => {
            reply(reply_sender, app_id, error_reply(stage, message));
        }
        Some(other) => {
            reply(
                reply_sender,
                app_id,
                SteamReply::SaveFailed(format!(
                    "unexpected StoreStats response: {:?}",
                    std::mem::discriminant(&other)
                )),
            );
        }
        None => {
            reply(
                reply_sender,
                app_id,
                SteamReply::SaveFailed("timed out waiting for StoreStats".to_owned()),
            );
        }
    }
}
