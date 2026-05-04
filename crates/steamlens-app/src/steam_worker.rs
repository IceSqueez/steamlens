use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc as async_mpsc;

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};
use steamlens_core::{AchievementIcon, GameSummary};

use crate::manager::types::ResetScope;

#[allow(dead_code)]
pub enum SteamRequest {
    ConnectWithApp(u32),
    ScanLibrary,
    RequestUserStats,
    RequestGlobalPercentages,
    ApplyChanges {
        achievements_to_set: Vec<String>,
        achievements_to_clear: Vec<String>,
        stats_int: HashMap<String, i32>,
        stats_float: HashMap<String, f32>,
    },
    ResetAll {
        scope: ResetScope,
        stat_driven_progress_max: HashMap<String, u32>,
    },
    Disconnect,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SteamReply {
    Connected {
        steam_id: u64,
        app_name: Option<String>,
    },
    ConnectFailed(String),
    LibraryScan(Vec<GameSummary>),
    LibraryScanFailed(String),
    StatsRequested,
    RequestStatsFailed(String),
    AchievementsAndStats {
        achievements: Vec<steamlens_core::AchievementData>,
        stats: Vec<steamlens_core::StatData>,
    },
    LoadFailed(String),
    ChangesSaved,
    SaveFailed(String),
    ResetDone,
    ResetFailed(String),
    IconUpdated {
        name: String,
        icon: AchievementIcon,
    },
    GlobalPercentagesReady(HashMap<String, f32>),
    GlobalPercentagesFailed(String),
    Callback(steamlens_core::SteamCallback),
    Disconnected,
}

pub struct SteamWorker {
    request_tx: async_mpsc::UnboundedSender<SteamRequest>,
}

impl SteamWorker {
    pub fn spawn() -> (Self, mpsc::Receiver<SteamReply>) {
        let (req_tx, req_rx) = async_mpsc::unbounded_channel::<SteamRequest>();
        let (rep_tx, rep_rx) = mpsc::channel::<SteamReply>();

        tokio::spawn(bridge_loop(req_rx, rep_tx));

        (SteamWorker { request_tx: req_tx }, rep_rx)
    }

    pub fn send(&self, req: SteamRequest) {
        let _ = self.request_tx.send(req);
    }

    #[cfg(test)]
    pub fn new_disconnected() -> Self {
        let (req_tx, _req_rx) = async_mpsc::unbounded_channel::<SteamRequest>();
        SteamWorker { request_tx: req_tx }
    }
}

fn reply(tx: &mpsc::Sender<SteamReply>, r: SteamReply) {
    let _ = tx.send(r);
}

/// Translates a single `SteamRequest` into the `WorkerCommand` sequence that
/// the child process must execute. `ScanLibrary`, `ConnectWithApp`, and
/// `Disconnect` are handled by the bridge loop directly and never reach this
/// function.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn translate_request(req: &SteamRequest) -> Vec<WorkerCommand> {
    match req {
        SteamRequest::RequestUserStats => vec![WorkerCommand::LoadAchievementsAndStats],

        SteamRequest::RequestGlobalPercentages => vec![WorkerCommand::RequestGlobalPercentages],

        SteamRequest::ApplyChanges {
            achievements_to_set,
            achievements_to_clear,
            stats_int,
            stats_float,
        } => {
            let mut cmds = Vec::new();
            for name in achievements_to_set {
                cmds.push(WorkerCommand::SetAchievement(name.clone()));
            }
            for name in achievements_to_clear {
                cmds.push(WorkerCommand::ClearAchievement(name.clone()));
            }
            for (name, value) in stats_int {
                cmds.push(WorkerCommand::SetStatInt {
                    name: name.clone(),
                    value: *value,
                });
            }
            for (name, value) in stats_float {
                cmds.push(WorkerCommand::SetStatFloat {
                    name: name.clone(),
                    value: *value,
                });
            }
            cmds.push(WorkerCommand::StoreStats);
            cmds
        }

        SteamRequest::ResetAll { scope, .. } => vec![WorkerCommand::ResetAllStats {
            include_achievements: *scope == ResetScope::StatsAndAchievements,
        }],

        SteamRequest::ConnectWithApp(_) | SteamRequest::ScanLibrary | SteamRequest::Disconnect => {
            vec![]
        }
    }
}

/// Maps a `WorkerResponse::Error { context, .. }` to the correct `SteamReply`
/// failure variant. The `context` strings come from the worker's error
/// reporting in `worker.rs` and the apply-sequence logic below.
fn error_reply(context: &str, message: String) -> SteamReply {
    match context {
        "connect" => SteamReply::ConnectFailed(message),
        "load" | "RequestUserStats" | "UserStatsReceived" | "num_achievements"
        | "poll_callbacks" => SteamReply::LoadFailed(message),
        "apply" | "StoreStats" | "UserStatsStored" => SteamReply::SaveFailed(message),
        "reset" | "ResetAllStats" => SteamReply::ResetFailed(message),
        "global_percentages"
        | "RequestGlobalPercentages"
        | "RequestGlobalAchievementPercentages"
        | "GlobalAchievementPercentages APICall"
        | "GlobalAchievementPercentagesReady"
        | "num_achievements (percentages)" => SteamReply::GlobalPercentagesFailed(message),
        _ => SteamReply::LoadFailed(format!("[{context}] {message}")),
    }
}

async fn read_response(stdout: &mut ChildStdout) -> Option<WorkerResponse> {
    let mut header = [0u8; 4];
    stdout.read_exact(&mut header).await.ok()?;
    let len = parse_header(header).ok()?;
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await.ok()?;
    decode_frame::<WorkerResponse>(&buf).ok()
}

async fn write_command(stdin: &mut ChildStdin, cmd: &WorkerCommand) -> bool {
    let Ok(framed) = encode_frame(cmd) else {
        return false;
    };
    stdin.write_all(&framed).await.is_ok() && stdin.flush().await.is_ok()
}

/// Sends a single command to the child and waits for exactly one response.
/// Returns `None` on timeout or I/O failure.
async fn round_trip(
    stdin: &mut ChildStdin,
    stdout: &mut ChildStdout,
    cmd: &WorkerCommand,
    timeout: Duration,
) -> Option<WorkerResponse> {
    if !write_command(stdin, cmd).await {
        return None;
    }
    tokio::time::timeout(timeout, read_response(stdout))
        .await
        .ok()
        .flatten()
}

/// Executes the `ApplyChanges` multi-command sequence:
/// Set/Clear/SetStat commands each get a 5 s timeout and must ack before the
/// next is sent. `StoreStats` gets a 15 s timeout (waits for UserStatsStored).
/// On any error or timeout the sequence aborts and `SaveFailed` is returned.
async fn run_apply_sequence(
    achievements_to_set: Vec<String>,
    achievements_to_clear: Vec<String>,
    stats_int: HashMap<String, i32>,
    stats_float: HashMap<String, f32>,
    stdin: &mut ChildStdin,
    stdout: &mut ChildStdout,
    rep_tx: &mpsc::Sender<SteamReply>,
) {
    let staging_timeout = Duration::from_secs(5);

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
        match round_trip(stdin, stdout, cmd, staging_timeout).await {
            Some(WorkerResponse::Ack) => {}
            Some(WorkerResponse::Error { context, message }) => {
                reply(rep_tx, error_reply(&context, message));
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

    let store_timeout = Duration::from_secs(15);
    match round_trip(stdin, stdout, &WorkerCommand::StoreStats, store_timeout).await {
        Some(WorkerResponse::Stored) => {
            reply(rep_tx, SteamReply::ChangesSaved);
            // After a successful store, re-load achievements+stats so the UI
            // reflects the new state (mirrors the old in-process worker behaviour).
            let load_timeout = Duration::from_secs(15);
            match round_trip(
                stdin,
                stdout,
                &WorkerCommand::LoadAchievementsAndStats,
                load_timeout,
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
        Some(WorkerResponse::Error { context, message }) => {
            reply(rep_tx, error_reply(&context, message));
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

/// Translates a `WorkerResponse` received from the child to a `SteamReply` and
/// sends it on `rep_tx`. Icon and data responses map 1:1; `Stored` maps to
/// `ChangesSaved`; `ResetDone` maps to `ResetDone`; errors are context-routed.
fn handle_worker_response(resp: WorkerResponse, rep_tx: &mpsc::Sender<SteamReply>) {
    match resp {
        WorkerResponse::Hello { steam_id, app_name } => {
            reply(rep_tx, SteamReply::Connected { steam_id, app_name });
        }
        WorkerResponse::Ack => {}
        WorkerResponse::AchievementsAndStats {
            achievements,
            stats,
        } => {
            reply(
                rep_tx,
                SteamReply::AchievementsAndStats {
                    achievements,
                    stats,
                },
            );
        }
        WorkerResponse::IconUpdated { name, icon } => {
            reply(rep_tx, SteamReply::IconUpdated { name, icon });
        }
        WorkerResponse::GlobalPercentagesReady(map) => {
            reply(rep_tx, SteamReply::GlobalPercentagesReady(map));
        }
        WorkerResponse::Stored => {
            reply(rep_tx, SteamReply::ChangesSaved);
        }
        WorkerResponse::ResetDone => {
            reply(rep_tx, SteamReply::ResetDone);
        }
        WorkerResponse::Error { context, message } => {
            reply(rep_tx, error_reply(&context, message));
        }
        WorkerResponse::Disconnected => {
            reply(rep_tx, SteamReply::Disconnected);
        }
    }
}

/// Drains any queued responses from `stdout` within `drain_ms` milliseconds.
/// Used after send to flush icon callbacks that the child emits asynchronously.
async fn drain_responses(
    stdout: &mut ChildStdout,
    rep_tx: &mpsc::Sender<SteamReply>,
    drain_ms: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(drain_ms);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, read_response(stdout)).await {
            Ok(Some(resp)) => handle_worker_response(resp, rep_tx),
            _ => break,
        }
    }
}

async fn kill_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
}

async fn bridge_loop(
    mut req_rx: async_mpsc::UnboundedReceiver<SteamRequest>,
    rep_tx: mpsc::Sender<SteamReply>,
) {
    // Phase 1: wait for the first request which must be `ConnectWithApp` or
    // a disk-only `ScanLibrary`. Any other request before connection gets a
    // "not connected" failure reply.
    let (mut child, mut stdin, mut stdout) = loop {
        let Some(req) = req_rx.recv().await else {
            return;
        };
        match req {
            SteamRequest::ScanLibrary => {
                handle_scan_library(&rep_tx);
                continue;
            }
            SteamRequest::ConnectWithApp(app_id) => {
                match spawn_worker_child(app_id).await {
                    Ok(tuple) => break tuple,
                    Err(e) => {
                        reply(&rep_tx, SteamReply::ConnectFailed(e.to_string()));
                        // Stay in the loop — parent might retry with another app_id.
                        continue;
                    }
                }
            }
            SteamRequest::Disconnect => {
                reply(&rep_tx, SteamReply::Disconnected);
                return;
            }
            _ => {
                reply(
                    &rep_tx,
                    SteamReply::RequestStatsFailed("Not connected".to_owned()),
                );
                continue;
            }
        }
    };

    // Phase 2: read the Hello from the child.
    let hello_timeout = Duration::from_secs(10);
    match tokio::time::timeout(hello_timeout, read_response(&mut stdout)).await {
        Ok(Some(WorkerResponse::Hello { steam_id, app_name })) => {
            reply(&rep_tx, SteamReply::Connected { steam_id, app_name });
        }
        Ok(Some(WorkerResponse::Error { context, message })) => {
            reply(&rep_tx, error_reply(&context, message));
            kill_child(&mut child).await;
            return;
        }
        Ok(other) => {
            reply(
                &rep_tx,
                SteamReply::ConnectFailed(format!(
                    "unexpected first message: {:?}",
                    other.as_ref().map(std::mem::discriminant)
                )),
            );
            kill_child(&mut child).await;
            return;
        }
        Err(_) => {
            reply(
                &rep_tx,
                SteamReply::ConnectFailed("timed out waiting for worker Hello".to_owned()),
            );
            kill_child(&mut child).await;
            return;
        }
    }

    // Phase 3: bidirectional command/response loop.
    loop {
        // Poll for icon callbacks or other async responses from the child (up
        // to 50 ms) before blocking on the next parent request.
        drain_responses(&mut stdout, &rep_tx, 50).await;

        let Some(req) = req_rx.recv().await else {
            // Parent dropped the SteamWorker — graceful shutdown.
            let _ = write_command(&mut stdin, &WorkerCommand::Shutdown).await;
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
            return;
        };

        match req {
            SteamRequest::ScanLibrary => {
                handle_scan_library(&rep_tx);
            }

            SteamRequest::ConnectWithApp(new_app_id) => {
                // Re-connect: send Shutdown to current child, reap it, spawn fresh.
                let _ = write_command(&mut stdin, &WorkerCommand::Shutdown).await;
                let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;

                match spawn_worker_child(new_app_id).await {
                    Ok((new_child, new_stdin, new_stdout)) => {
                        child = new_child;
                        stdin = new_stdin;
                        stdout = new_stdout;
                        // Read Hello from fresh child.
                        let hello_timeout = Duration::from_secs(10);
                        match tokio::time::timeout(hello_timeout, read_response(&mut stdout)).await
                        {
                            Ok(Some(WorkerResponse::Hello { steam_id, app_name })) => {
                                reply(&rep_tx, SteamReply::Connected { steam_id, app_name });
                            }
                            Ok(Some(WorkerResponse::Error { context, message })) => {
                                reply(&rep_tx, error_reply(&context, message));
                                kill_child(&mut child).await;
                                return;
                            }
                            _ => {
                                reply(
                                    &rep_tx,
                                    SteamReply::ConnectFailed(
                                        "timed out waiting for worker Hello on reconnect"
                                            .to_owned(),
                                    ),
                                );
                                kill_child(&mut child).await;
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        reply(&rep_tx, SteamReply::ConnectFailed(e.to_string()));
                        return;
                    }
                }
            }

            SteamRequest::Disconnect => {
                let _ = write_command(&mut stdin, &WorkerCommand::Shutdown).await;
                // Read Disconnected acknowledgment with a short timeout.
                let _ =
                    tokio::time::timeout(Duration::from_secs(3), read_response(&mut stdout)).await;
                let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
                reply(&rep_tx, SteamReply::Disconnected);
                return;
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
                    &mut stdin,
                    &mut stdout,
                    &rep_tx,
                )
                .await;
            }

            SteamRequest::RequestUserStats => {
                let timeout = Duration::from_secs(15);
                match round_trip(
                    &mut stdin,
                    &mut stdout,
                    &WorkerCommand::LoadAchievementsAndStats,
                    timeout,
                )
                .await
                {
                    Some(resp) => handle_worker_response(resp, &rep_tx),
                    None => reply(
                        &rep_tx,
                        SteamReply::LoadFailed(
                            "timed out waiting for AchievementsAndStats".to_owned(),
                        ),
                    ),
                }
            }

            SteamRequest::RequestGlobalPercentages => {
                let timeout = Duration::from_secs(15);
                match round_trip(
                    &mut stdin,
                    &mut stdout,
                    &WorkerCommand::RequestGlobalPercentages,
                    timeout,
                )
                .await
                {
                    Some(resp) => handle_worker_response(resp, &rep_tx),
                    None => reply(
                        &rep_tx,
                        SteamReply::GlobalPercentagesFailed(
                            "timed out waiting for global percentages".to_owned(),
                        ),
                    ),
                }
            }

            SteamRequest::ResetAll { scope, .. } => {
                let include_achievements = scope == ResetScope::StatsAndAchievements;
                let timeout = Duration::from_secs(15);
                match round_trip(
                    &mut stdin,
                    &mut stdout,
                    &WorkerCommand::ResetAllStats {
                        include_achievements,
                    },
                    timeout,
                )
                .await
                {
                    Some(resp) => handle_worker_response(resp, &rep_tx),
                    None => reply(
                        &rep_tx,
                        SteamReply::ResetFailed("timed out waiting for ResetAllStats".to_owned()),
                    ),
                }
            }
        }
    }
}

async fn spawn_worker_child(
    app_id: u32,
) -> Result<(Child, ChildStdin, ChildStdout), std::io::Error> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .arg("--worker")
        .arg(app_id.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdin missing")
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdout missing")
    })?;
    Ok((child, stdin, stdout))
}

fn handle_scan_library(rep_tx: &mpsc::Sender<SteamReply>) {
    match steamlens_core::scan_installed_games() {
        Ok(games) => reply(rep_tx, SteamReply::LibraryScan(games)),
        Err(e) => reply(rep_tx, SteamReply::LibraryScanFailed(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_apply(
        set: &[&str],
        clear: &[&str],
        ints: &[(&str, i32)],
        floats: &[(&str, f32)],
    ) -> SteamRequest {
        SteamRequest::ApplyChanges {
            achievements_to_set: set.iter().map(|s| s.to_string()).collect(),
            achievements_to_clear: clear.iter().map(|s| s.to_string()).collect(),
            stats_int: ints.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            stats_float: floats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn translate_request_user_stats() {
        let cmds = translate_request(&SteamRequest::RequestUserStats);
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], WorkerCommand::LoadAchievementsAndStats));
    }

    #[test]
    fn translate_request_global_percentages() {
        let cmds = translate_request(&SteamRequest::RequestGlobalPercentages);
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], WorkerCommand::RequestGlobalPercentages));
    }

    #[test]
    fn translate_request_reset_stats_only() {
        let req = SteamRequest::ResetAll {
            scope: ResetScope::StatsOnly,
            stat_driven_progress_max: HashMap::new(),
        };
        let cmds = translate_request(&req);
        assert_eq!(cmds.len(), 1);
        assert!(
            matches!(
                cmds[0],
                WorkerCommand::ResetAllStats {
                    include_achievements: false
                }
            ),
            "StatsOnly must produce include_achievements=false"
        );
    }

    #[test]
    fn translate_request_reset_stats_and_achievements() {
        let req = SteamRequest::ResetAll {
            scope: ResetScope::StatsAndAchievements,
            stat_driven_progress_max: HashMap::new(),
        };
        let cmds = translate_request(&req);
        assert_eq!(cmds.len(), 1);
        assert!(
            matches!(
                cmds[0],
                WorkerCommand::ResetAllStats {
                    include_achievements: true
                }
            ),
            "StatsAndAchievements must produce include_achievements=true"
        );
    }

    #[test]
    fn translate_request_apply_changes_ordering() {
        let req = make_apply(
            &["ACH_A", "ACH_B"],
            &["ACH_C"],
            &[("kills", 10)],
            &[("ratio", 1.5)],
        );
        let cmds = translate_request(&req);
        // Expected: SetAchievement x2, ClearAchievement x1, SetStatInt x1, SetStatFloat x1, StoreStats.
        assert_eq!(cmds.len(), 6);
        assert!(matches!(&cmds[0], WorkerCommand::SetAchievement(n) if n == "ACH_A"));
        assert!(matches!(&cmds[1], WorkerCommand::SetAchievement(n) if n == "ACH_B"));
        assert!(matches!(&cmds[2], WorkerCommand::ClearAchievement(n) if n == "ACH_C"));
        // SetStatInt or SetStatFloat can appear in any order (from HashMap iteration).
        let has_int = cmds[3..5]
            .iter()
            .any(|c| matches!(c, WorkerCommand::SetStatInt { name, value } if name == "kills" && *value == 10));
        let has_float = cmds[3..5]
            .iter()
            .any(|c| matches!(c, WorkerCommand::SetStatFloat { name, value } if name == "ratio" && (*value - 1.5).abs() < f32::EPSILON));
        assert!(has_int, "SetStatInt(kills,10) must be present");
        assert!(has_float, "SetStatFloat(ratio,1.5) must be present");
        assert!(matches!(&cmds[5], WorkerCommand::StoreStats));
    }

    #[test]
    fn translate_request_apply_empty_changes_still_has_store_stats() {
        let req = make_apply(&[], &[], &[], &[]);
        let cmds = translate_request(&req);
        assert_eq!(cmds.len(), 1, "empty apply must still produce StoreStats");
        assert!(matches!(cmds[0], WorkerCommand::StoreStats));
    }

    #[test]
    fn translate_request_disconnect_produces_no_commands() {
        let cmds = translate_request(&SteamRequest::Disconnect);
        assert!(cmds.is_empty());
    }

    #[test]
    fn translate_request_scan_library_produces_no_commands() {
        let cmds = translate_request(&SteamRequest::ScanLibrary);
        assert!(cmds.is_empty());
    }
}
