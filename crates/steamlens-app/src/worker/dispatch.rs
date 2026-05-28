use std::time::Duration;

use steamlens_core::Client;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorKind, WorkerResponse};
use tokio::sync::mpsc;

use super::callbacks::forward_icon_callbacks;
use super::commands::{
    fetch_global_percentages, load_achievements_and_stats, load_achievements_card_only,
    quick_achievement_count, store_stats_and_wait,
};
use super::error::WorkerError;
use super::ipc_io::{read_command, write_response};

pub(super) enum DispatchOutcome {
    Continue,
    Shutdown,
    Fatal,
}

pub(super) async fn dispatch_loop(client: Client, app_id: u32) -> i32 {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Result<Option<WorkerCommand>, WorkerError>>(1);

    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        loop {
            let result = read_command(&mut stdin).await;
            let stop = matches!(result, Err(_) | Ok(None));
            if cmd_tx.send(result).await.is_err() {
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

            cmd = cmd_rx.recv() => {
                let cmd = match cmd {
                    Some(c) => c,
                    None => {
                        tracing::debug!("stdin reader task ended");
                        return 1;
                    }
                };
                match cmd {
                    Err(e) => {
                        tracing::error!("read_command err: {e}");
                        let _ = write_response(&WorkerResponse::Error {
                            kind: WorkerErrorKind::Generic,
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
                            "cmd received: {}",
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

fn command_label(cmd: &WorkerCommand) -> &'static str {
    match cmd {
        WorkerCommand::LoadAchievementsAndStats => "LoadAchievementsAndStats",
        WorkerCommand::LoadAchievementsAndStatsCardOnly => "LoadAchievementsAndStatsCardOnly",
        WorkerCommand::QuickAchievementCount => "QuickAchievementCount",
        WorkerCommand::StoreStats => "StoreStats",
        WorkerCommand::RequestGlobalPercentages => "RequestGlobalPercentages",
        WorkerCommand::SetAchievement(_) => "SetAchievement",
        WorkerCommand::ClearAchievement(_) => "ClearAchievement",
        WorkerCommand::SetStatInt { .. } => "SetStatInt",
        WorkerCommand::SetStatFloat { .. } => "SetStatFloat",
        WorkerCommand::Shutdown => "Shutdown",
    }
}

async fn handle_command(cmd: WorkerCommand, client: &Client, app_id: u32) -> DispatchOutcome {
    match cmd {
        WorkerCommand::LoadAchievementsAndStats => {
            let resp = load_achievements_and_stats(client, app_id).await;
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
            let pct = fetch_global_percentages(client).await;
            if write_response(&pct).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetAchievement(name) => {
            let resp = match client.user_stats().set_achievement(&name) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    kind: WorkerErrorKind::Generic,
                    message: format!("SetAchievement({name}): {e}"),
                },
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::ClearAchievement(name) => {
            let resp = match client.user_stats().clear_achievement(&name) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    kind: WorkerErrorKind::Generic,
                    message: format!("ClearAchievement({name}): {e}"),
                },
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetStatInt { name, value } => {
            let resp = match client.user_stats().set_stat_int(&name, value) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    kind: WorkerErrorKind::Generic,
                    message: format!("SetStatInt({name}): {e}"),
                },
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetStatFloat { name, value } => {
            let resp = match client.user_stats().set_stat_float(&name, value) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    kind: WorkerErrorKind::Generic,
                    message: format!("SetStatFloat({name}): {e}"),
                },
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::StoreStats => {
            let resp = store_stats_and_wait(client).await;
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::RequestGlobalPercentages => {
            let resp = fetch_global_percentages(client).await;
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::QuickAchievementCount => {
            let resp = quick_achievement_count(client).await;
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::LoadAchievementsAndStatsCardOnly => {
            let resp = load_achievements_card_only(client, app_id).await;
            if write_response(&resp).await.is_err() {
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
