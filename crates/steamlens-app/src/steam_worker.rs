use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc as async_mpsc;

use steamlens_core::AchievementIcon;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorKind, WorkerResponse, encode_frame};

use crate::game_view::types::ResetScope;
use crate::ipc_pipe;
use crate::timeouts;

pub enum SteamRequest {
    ConnectWithApp(u32),
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
    },
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum SteamReply {
    Connected {
        app_name: Option<String>,
    },
    ConnectFailed(String),
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
    GlobalPercentagesFailed,
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
}

fn reply(tx: &mpsc::Sender<SteamReply>, r: SteamReply) {
    let _ = tx.send(r);
}

#[cfg(test)]
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

        SteamRequest::ConnectWithApp(_) | SteamRequest::Disconnect => {
            vec![]
        }
    }
}

fn error_reply(kind: WorkerErrorKind, message: String) -> SteamReply {
    match kind {
        WorkerErrorKind::Connect => SteamReply::ConnectFailed(message),
        WorkerErrorKind::RequestUserStats
        | WorkerErrorKind::UserStatsReceived
        | WorkerErrorKind::PollCallbacks => SteamReply::LoadFailed(message),
        WorkerErrorKind::StoreStats | WorkerErrorKind::UserStatsStored => {
            SteamReply::SaveFailed(message)
        }
        WorkerErrorKind::ResetAllStats => SteamReply::ResetFailed(message),
        WorkerErrorKind::RequestGlobalPercentages
        | WorkerErrorKind::GlobalPercentagesReady
        | WorkerErrorKind::GlobalPercentagesAPICall => SteamReply::GlobalPercentagesFailed,
        WorkerErrorKind::Generic => SteamReply::LoadFailed(message),
    }
}

async fn write_command(stdin: &mut ChildStdin, cmd: &WorkerCommand) -> bool {
    let Ok(framed) = encode_frame(cmd) else {
        return false;
    };
    stdin.write_all(&framed).await.is_ok() && stdin.flush().await.is_ok()
}

async fn round_trip(
    stdin: &mut ChildStdin,
    stdout: &mut ChildStdout,
    cmd: &WorkerCommand,
    timeout: Duration,
) -> Option<WorkerResponse> {
    if !write_command(stdin, cmd).await {
        return None;
    }
    tokio::time::timeout(timeout, ipc_pipe::read_response(stdout))
        .await
        .ok()
        .flatten()
}

async fn run_apply_sequence(
    achievements_to_set: Vec<String>,
    achievements_to_clear: Vec<String>,
    stats_int: HashMap<String, i32>,
    stats_float: HashMap<String, f32>,
    stdin: &mut ChildStdin,
    stdout: &mut ChildStdout,
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
        match round_trip(stdin, stdout, cmd, staging_timeout).await {
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
    match round_trip(stdin, stdout, &WorkerCommand::StoreStats, store_timeout).await {
        Some(WorkerResponse::Stored) => {
            reply(rep_tx, SteamReply::ChangesSaved);
            let load_timeout = timeouts::LIVE_LOAD;
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

fn read_shm<T: serde::de::DeserializeOwned>(
    label: &str,
    shm_path: &str,
    region_bytes: u64,
) -> Result<T, String> {
    let path = std::path::PathBuf::from(shm_path);
    steamlens_core::read_payload::<T>(&path, region_bytes)
        .map_err(|e| format!("{label} shm read at {}: {e}", path.display()))
}

fn handle_worker_response(resp: WorkerResponse, rep_tx: &mpsc::Sender<SteamReply>) {
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
                eprintln!("[steamlens] icon shm read failed for {name}: {msg}");
            }
        },
        WorkerResponse::GlobalPercentagesReady {
            shm_path,
            region_bytes,
        } => match read_shm::<std::collections::HashMap<String, f32>>(
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
        WorkerResponse::ResetDone => {
            reply(rep_tx, SteamReply::ResetDone);
        }
        WorkerResponse::AchievementCount {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
        }
        WorkerResponse::ProbeResult {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
        }
        WorkerResponse::CardOnlyAchievements {
            shm_path,
            region_bytes: _,
        } => {
            steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
        }
        WorkerResponse::Error { kind, message } => {
            reply(rep_tx, error_reply(kind, message));
        }
        WorkerResponse::Disconnected => {
            reply(rep_tx, SteamReply::Disconnected);
        }
    }
}

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
        match tokio::time::timeout(remaining, ipc_pipe::read_response(stdout)).await {
            Ok(Some(resp)) => handle_worker_response(resp, rep_tx),
            _ => break,
        }
    }
}

async fn kill_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(timeouts::CHILD_KILL, child.wait()).await;
}

#[derive(Debug)]
enum ConnectError {
    WorkerError(WorkerErrorKind, String),
    UnexpectedMessage,
    Timeout,
}

async fn await_steam_connected(
    stdout: &mut ChildStdout,
    timeout: Duration,
    rep_tx: &mpsc::Sender<SteamReply>,
) -> Result<(), ConnectError> {
    match tokio::time::timeout(timeout, ipc_pipe::read_response(stdout)).await {
        Ok(Some(WorkerResponse::SteamConnected { app_name, .. })) => {
            reply(rep_tx, SteamReply::Connected { app_name });
            Ok(())
        }
        Ok(Some(WorkerResponse::Error { kind, message })) => {
            Err(ConnectError::WorkerError(kind, message))
        }
        Ok(_) => Err(ConnectError::UnexpectedMessage),
        Err(_) => Err(ConnectError::Timeout),
    }
}

async fn handle_request(
    req: SteamRequest,
    stdin: &mut ChildStdin,
    stdout: &mut ChildStdout,
    rep_tx: &mpsc::Sender<SteamReply>,
) {
    match req {
        SteamRequest::RequestUserStats => {
            let timeout = timeouts::LIVE_LOAD;
            match round_trip(
                stdin,
                stdout,
                &WorkerCommand::LoadAchievementsAndStats,
                timeout,
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
                stdin,
                stdout,
                &WorkerCommand::RequestGlobalPercentages,
                timeout,
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
                stdin,
                stdout,
                rep_tx,
            )
            .await;
        }

        SteamRequest::ResetAll { scope, .. } => {
            let include_achievements = scope == ResetScope::StatsAndAchievements;
            let timeout = timeouts::LIVE_LOAD;
            match round_trip(
                stdin,
                stdout,
                &WorkerCommand::ResetAllStats {
                    include_achievements,
                },
                timeout,
            )
            .await
            {
                Some(resp) => handle_worker_response(resp, rep_tx),
                None => reply(
                    rep_tx,
                    SteamReply::ResetFailed("timed out waiting for ResetAllStats".to_owned()),
                ),
            }
        }

        SteamRequest::ConnectWithApp(_) | SteamRequest::Disconnect => {}
    }
}

async fn bridge_loop(
    mut req_rx: async_mpsc::UnboundedReceiver<SteamRequest>,
    rep_tx: mpsc::Sender<SteamReply>,
) {
    let connect_timeout = timeouts::STEAM_CONNECT;

    let (mut child, mut stdin, mut stdout, mut _job_guard) = loop {
        let Some(req) = req_rx.recv().await else {
            return;
        };
        match req {
            SteamRequest::ConnectWithApp(app_id) => match spawn_worker_child(app_id).await {
                Ok(tuple) => break tuple,
                Err(e) => {
                    reply(&rep_tx, SteamReply::ConnectFailed(e.to_string()));
                    continue;
                }
            },
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

    if let Err(e) = await_steam_connected(&mut stdout, connect_timeout, &rep_tx).await {
        match e {
            ConnectError::WorkerError(kind, msg) => reply(&rep_tx, error_reply(kind, msg)),
            ConnectError::UnexpectedMessage => reply(
                &rep_tx,
                SteamReply::ConnectFailed("unexpected first message from worker".to_owned()),
            ),
            ConnectError::Timeout => reply(
                &rep_tx,
                SteamReply::ConnectFailed("timed out waiting for SteamConnected".to_owned()),
            ),
        }
        kill_child(&mut child).await;
        return;
    }

    loop {
        // Drain icon callbacks (≤50 ms) before blocking on the next request.
        drain_responses(&mut stdout, &rep_tx, timeouts::ICON_DRAIN_MS).await;

        let Some(req) = req_rx.recv().await else {
            let _ = write_command(&mut stdin, &WorkerCommand::Shutdown).await;
            let _ = tokio::time::timeout(timeouts::CHILD_DRAIN, child.wait()).await;
            return;
        };

        match req {
            SteamRequest::ConnectWithApp(new_app_id) => {
                let _ = write_command(&mut stdin, &WorkerCommand::Shutdown).await;
                let _ = tokio::time::timeout(timeouts::CHILD_DRAIN, child.wait()).await;

                match spawn_worker_child(new_app_id).await {
                    Ok((new_child, new_stdin, new_stdout, new_job_guard)) => {
                        child = new_child;
                        stdin = new_stdin;
                        stdout = new_stdout;
                        _job_guard = new_job_guard;

                        if let Err(e) =
                            await_steam_connected(&mut stdout, connect_timeout, &rep_tx).await
                        {
                            match e {
                                ConnectError::WorkerError(kind, msg) => {
                                    reply(&rep_tx, error_reply(kind, msg))
                                }
                                ConnectError::UnexpectedMessage | ConnectError::Timeout => reply(
                                    &rep_tx,
                                    SteamReply::ConnectFailed(
                                        "timed out waiting for SteamConnected on reconnect"
                                            .to_owned(),
                                    ),
                                ),
                            }
                            kill_child(&mut child).await;
                            return;
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
                let _ = tokio::time::timeout(
                    timeouts::CHILD_DRAIN,
                    ipc_pipe::read_response(&mut stdout),
                )
                .await;
                let _ = tokio::time::timeout(timeouts::CHILD_DRAIN, child.wait()).await;
                reply(&rep_tx, SteamReply::Disconnected);
                return;
            }

            other => {
                handle_request(other, &mut stdin, &mut stdout, &rep_tx).await;
            }
        }
    }
}

async fn spawn_worker_child(
    app_id: u32,
) -> Result<
    (
        Child,
        ChildStdin,
        ChildStdout,
        steamlens_core::ChildLifetimeGuard,
    ),
    std::io::Error,
> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .arg("--worker")
        .arg(app_id.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let pid = child
        .id()
        .ok_or_else(|| std::io::Error::other("spawned worker has no pid"))?;
    let job_guard = steamlens_core::associate_kill_on_parent_exit(pid).inspect_err(|_| {
        let _ = child.start_kill();
    })?;

    let stdin = child.stdin.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdin missing")
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdout missing")
    })?;
    Ok((child, stdin, stdout, job_guard))
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
        assert_eq!(cmds.len(), 6);
        assert!(matches!(&cmds[0], WorkerCommand::SetAchievement(n) if n == "ACH_A"));
        assert!(matches!(&cmds[1], WorkerCommand::SetAchievement(n) if n == "ACH_B"));
        assert!(matches!(&cmds[2], WorkerCommand::ClearAchievement(n) if n == "ACH_C"));
        // SetStatInt and SetStatFloat order varies (HashMap iteration).
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

    // ── await_steam_connected unit tests ────────────────────────────────────
    //
    // ChildStdout is not constructable directly. We test the underlying
    // `read_response` logic via a tokio DuplexStream that has the same
    // AsyncRead contract.

    use steamlens_core::ipc::{WorkerResponse, decode_frame, encode_frame, parse_header};

    async fn read_from_duplex(mut rx: tokio::io::DuplexStream) -> Option<WorkerResponse> {
        use tokio::io::AsyncReadExt as _;

        let mut header = [0u8; 4];
        rx.read_exact(&mut header).await.ok()?;
        let len = parse_header(header).ok()?;
        let mut buf = vec![0u8; len];
        rx.read_exact(&mut buf).await.ok()?;
        decode_frame::<WorkerResponse>(&buf).ok()
    }

    #[tokio::test]
    async fn await_steam_connected_happy_path() {
        use tokio::io::AsyncWriteExt as _;

        let (mut tx, rx) = tokio::io::duplex(4096);
        let resp = WorkerResponse::SteamConnected {
            steam_id: 12345,
            app_name: Some("TestGame".to_owned()),
        };
        tx.write_all(&encode_frame(&resp).unwrap()).await.unwrap();
        drop(tx);

        let result = read_from_duplex(rx).await;
        assert!(matches!(
            result,
            Some(WorkerResponse::SteamConnected {
                steam_id: 12345,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn await_steam_connected_timeout() {
        let (_tx, rx) = tokio::io::duplex(4096);
        let timed_out = tokio::time::timeout(timeouts::POLL_INTERVAL, read_from_duplex(rx))
            .await
            .is_err();
        assert!(timed_out, "read on empty pipe must time out");
    }

    #[tokio::test]
    async fn await_steam_connected_worker_error() {
        use tokio::io::AsyncWriteExt as _;

        let (mut tx, rx) = tokio::io::duplex(4096);
        let resp = WorkerResponse::Error {
            kind: WorkerErrorKind::Connect,
            message: "steam not running".to_owned(),
        };
        tx.write_all(&encode_frame(&resp).unwrap()).await.unwrap();
        drop(tx);

        let result = read_from_duplex(rx).await;
        assert!(matches!(
            result,
            Some(WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                ..
            })
        ));
    }
}
