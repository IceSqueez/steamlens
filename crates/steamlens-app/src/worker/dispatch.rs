use std::time::Duration;

use steamlens_core::Client;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorStage, WorkerResponse};
use tokio::sync::mpsc;

use super::callbacks::forward_icon_callbacks;
use super::commands::{
    fetch_global_percentages, load_achievement_count, load_achievements_and_stats,
    load_achievements_summary, store_stats_and_wait,
};
use super::error::WorkerError;
use super::ipc_io::{read_command, write_response};

pub(super) enum DispatchOutcome {
    Continue,
    Shutdown,
    Fatal,
}

pub(super) async fn dispatch_loop(client: Client, app_id: u32) -> i32 {
    let (command_sender, mut command_receiver) =
        mpsc::channel::<Result<Option<WorkerCommand>, WorkerError>>(1);

    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        loop {
            let result = read_command(&mut stdin).await;
            let stop = matches!(result, Err(_) | Ok(None));
            if command_sender.send(result).await.is_err() {
                break;
            }
            if stop {
                break;
            }
        }
    });

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::debug!("dispatch_loop ready");

    loop {
        tokio::select! {
            biased;

            command = command_receiver.recv() => {
                let command = match command {
                    Some(c) => c,
                    None => {
                        tracing::debug!("stdin reader task ended");
                        return 1;
                    }
                };
                match command {
                    Err(e) => {
                        tracing::error!("read_command err: {e}");
                        let _ = write_response(&WorkerResponse::Error {
                            stage: WorkerErrorStage::Generic,
                            message: e.to_string(),
                        }).await;
                        return 1;
                    }
                    Ok(None) => {
                        tracing::debug!("stdin EOF");
                        return 0;
                    }
                    Ok(Some(command)) => {
                        tracing::debug!(
                            "command received: {}",
                            command_label(&command)
                        );
                        match handle_command(command, &client, app_id).await {
                            DispatchOutcome::Continue => {}
                            DispatchOutcome::Shutdown => return 0,
                            DispatchOutcome::Fatal => return 1,
                        }
                    }
                }
            }

            _ = interval.tick() => {
                if let Ok(callbacks) = client.poll_callbacks() {
                    if !callbacks.is_empty() {
                        tracing::trace!(
                            "tick — {} callbacks",
                            callbacks.len()
                        );
                    }
                    forward_icon_callbacks(callbacks, &client).await;
                }
            }
        }
    }
}

fn command_label(command: &WorkerCommand) -> &'static str {
    match command {
        WorkerCommand::LoadAchievementsFull => "LoadAchievementsFull",
        WorkerCommand::LoadAchievementsSummary => "LoadAchievementsSummary",
        WorkerCommand::LoadAchievementsCount => "LoadAchievementsCount",
        WorkerCommand::StoreStats => "StoreStats",
        WorkerCommand::RequestGlobalPercentages => "RequestGlobalPercentages",
        WorkerCommand::SetAchievement(_) => "SetAchievement",
        WorkerCommand::ClearAchievement(_) => "ClearAchievement",
        WorkerCommand::SetStatInt { .. } => "SetStatInt",
        WorkerCommand::SetStatFloat { .. } => "SetStatFloat",
        WorkerCommand::Shutdown => "Shutdown",
    }
}

async fn handle_command(command: WorkerCommand, client: &Client, app_id: u32) -> DispatchOutcome {
    match command {
        WorkerCommand::LoadAchievementsFull => {
            let response = load_achievements_and_stats(client, app_id).await;
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
            let percent = fetch_global_percentages(client).await;
            if write_response(&percent).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetAchievement(name) => {
            let response = match client.user_stats().set_achievement(&name) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    stage: WorkerErrorStage::Generic,
                    message: format!("SetAchievement({name}): {e}"),
                },
            };
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::ClearAchievement(name) => {
            let response = match client.user_stats().clear_achievement(&name) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    stage: WorkerErrorStage::Generic,
                    message: format!("ClearAchievement({name}): {e}"),
                },
            };
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetStatInt { name, value } => {
            let response = match client.user_stats().set_stat_int(&name, value) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    stage: WorkerErrorStage::Generic,
                    message: format!("SetStatInt({name}): {e}"),
                },
            };
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetStatFloat { name, value } => {
            let response = match client.user_stats().set_stat_float(&name, value) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    stage: WorkerErrorStage::Generic,
                    message: format!("SetStatFloat({name}): {e}"),
                },
            };
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::StoreStats => {
            let response = store_stats_and_wait(client).await;
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::RequestGlobalPercentages => {
            let response = fetch_global_percentages(client).await;
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::LoadAchievementsCount => {
            let response = load_achievement_count(client).await;
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::LoadAchievementsSummary => {
            let response = load_achievements_summary(client, app_id).await;
            if write_response(&response).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::Shutdown => {
            let _ = write_response(&WorkerResponse::Disconnected).await;
            return DispatchOutcome::Shutdown;
        }
    }
    DispatchOutcome::Continue
}
