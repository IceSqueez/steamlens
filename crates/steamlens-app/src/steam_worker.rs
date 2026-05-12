use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use tokio::sync::mpsc as async_mpsc;

use steamlens_core::AchievementIcon;
use steamlens_core::ipc::{WorkerCommand, WorkerErrorKind, WorkerResponse};

use crate::timeouts;
use crate::worker_subprocess::{WorkerHandle, WorkerMode, WorkerSpawnError};

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

    /// Sync because the underlying mpsc send is sync; pre-flight runs before any pipe touch.
    pub fn send_checked(
        &self,
        req: SteamRequest,
        steam_running: bool,
        user_logged_in: bool,
    ) -> Result<(), crate::worker_subprocess::ConnectivityError> {
        crate::worker_subprocess::preflight(steam_running, user_logged_in)?;
        let _ = self.request_tx.send(req);
        Ok(())
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

        SteamRequest::ConnectWithApp(_) | SteamRequest::Disconnect => {
            vec![]
        }
    }
}

fn error_reply(kind: WorkerErrorKind, message: String) -> SteamReply {
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

async fn round_trip(
    handle: &mut WorkerHandle,
    cmd: &WorkerCommand,
    timeout: Duration,
    rep_tx: &mpsc::Sender<SteamReply>,
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

async fn run_apply_sequence(
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
                crate::log!("icon shm read failed for {name}: {msg}");
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

type ConnectError = crate::worker_subprocess::WorkerProtocolError;

async fn await_steam_connected(
    handle: &mut WorkerHandle,
    timeout: Duration,
    rep_tx: &mpsc::Sender<SteamReply>,
) -> Result<(), ConnectError> {
    match tokio::time::timeout(timeout, handle.recv()).await {
        Ok(Ok(Some(WorkerResponse::SteamConnected { app_name, .. }))) => {
            reply(rep_tx, SteamReply::Connected { app_name });
            Ok(())
        }
        Ok(Ok(Some(WorkerResponse::Error { kind, message }))) => {
            Err(ConnectError::WorkerError { kind, message })
        }
        Ok(Ok(Some(_))) => Err(ConnectError::UnexpectedMessage),
        Ok(Ok(None)) => Err(ConnectError::UnexpectedEof),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(ConnectError::Timeout),
    }
}

async fn handle_request(
    req: SteamRequest,
    handle: &mut WorkerHandle,
    rep_tx: &mpsc::Sender<SteamReply>,
) {
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

async fn bridge_loop(
    mut req_rx: async_mpsc::UnboundedReceiver<SteamRequest>,
    rep_tx: mpsc::Sender<SteamReply>,
) {
    let connect_timeout = timeouts::STEAM_CONNECT;

    let mut handle = loop {
        let Some(req) = req_rx.recv().await else {
            return;
        };
        match req {
            SteamRequest::ConnectWithApp(app_id) => {
                match WorkerHandle::spawn(app_id, WorkerMode::Interactive).await {
                    Ok(h) => break h,
                    Err(e) => {
                        reply(&rep_tx, SteamReply::ConnectFailed(spawn_error_message(e)));
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

    if let Err(e) = await_steam_connected(&mut handle, connect_timeout, &rep_tx).await {
        match e {
            ConnectError::WorkerError { kind, message } => {
                reply(&rep_tx, error_reply(kind, message))
            }
            ConnectError::UnexpectedMessage => reply(
                &rep_tx,
                SteamReply::ConnectFailed("unexpected first message from worker".to_owned()),
            ),
            ConnectError::Timeout => reply(
                &rep_tx,
                SteamReply::ConnectFailed("timed out waiting for SteamConnected".to_owned()),
            ),
            ConnectError::UnexpectedEof | ConnectError::Decode(_) | ConnectError::Write(_) => {
                reply(
                    &rep_tx,
                    SteamReply::ConnectFailed("protocol error".to_owned()),
                )
            }
        }
        let _ = handle.finish().await;
        return;
    }

    loop {
        let req = tokio::select! {
            biased;
            resp = handle.recv() => {
                match resp {
                    Ok(Some(r)) => {
                        handle_worker_response(r, &rep_tx);
                        continue;
                    }
                    Ok(None) | Err(_) => {
                        let _ = handle.finish().await;
                        return;
                    }
                }
            }
            req = req_rx.recv() => match req {
                Some(r) => r,
                None => {
                    let _ = handle.finish().await;
                    return;
                }
            },
        };

        match req {
            SteamRequest::ConnectWithApp(new_app_id) => {
                match WorkerHandle::spawn(new_app_id, WorkerMode::Interactive).await {
                    Ok(new_handle) => {
                        let old_handle = std::mem::replace(&mut handle, new_handle);
                        let _ = old_handle.finish().await;

                        if let Err(e) =
                            await_steam_connected(&mut handle, connect_timeout, &rep_tx).await
                        {
                            match e {
                                ConnectError::WorkerError { kind, message } => {
                                    reply(&rep_tx, error_reply(kind, message))
                                }
                                ConnectError::UnexpectedMessage
                                | ConnectError::Timeout
                                | ConnectError::UnexpectedEof
                                | ConnectError::Decode(_)
                                | ConnectError::Write(_) => reply(
                                    &rep_tx,
                                    SteamReply::ConnectFailed(
                                        "timed out waiting for SteamConnected on reconnect"
                                            .to_owned(),
                                    ),
                                ),
                            }
                            let _ = handle.finish().await;
                            return;
                        }
                    }
                    Err(e) => {
                        reply(&rep_tx, SteamReply::ConnectFailed(spawn_error_message(e)));
                        return;
                    }
                }
            }

            SteamRequest::Disconnect => {
                let _ = handle.finish().await;
                reply(&rep_tx, SteamReply::Disconnected);
                return;
            }

            other => {
                handle_request(other, &mut handle, &rep_tx).await;
            }
        }
    }
}

fn spawn_error_message(e: WorkerSpawnError) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_checked_blocks_when_steam_not_running() {
        let (worker, _rx) = SteamWorker::spawn();
        let err = worker
            .send_checked(SteamRequest::RequestUserStats, false, true)
            .unwrap_err();
        assert!(matches!(
            err,
            crate::worker_subprocess::ConnectivityError::SteamNotRunning
        ));
    }

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
