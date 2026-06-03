mod api;
mod apply;
mod bridge;
mod dispatch;

pub use api::{SteamReply, SteamRequest, SteamWorker};

#[cfg(test)]
pub(crate) fn translate_request(req: &SteamRequest) -> Vec<steamlens_core::ipc::WorkerCommand> {
    use steamlens_core::ipc::WorkerCommand;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use steamlens_core::ipc::{WorkerCommand, WorkerErrorStage};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn dispatch_checked_blocks_when_steam_not_running() {
        let (reply_tx, _reply_rx) = mpsc::unbounded_channel();
        let worker = SteamWorker::spawn(reply_tx);
        let err = worker
            .dispatch_checked(SteamRequest::RequestUserStats, false, true, ())
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
            stats_int: ints
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<HashMap<_, _>>(),
            stats_float: floats
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<HashMap<_, _>>(),
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
        let timed_out = tokio::time::timeout(crate::timeouts::POLL_INTERVAL, read_from_duplex(rx))
            .await
            .is_err();
        assert!(timed_out, "read on empty pipe must time out");
    }

    #[tokio::test]
    async fn await_steam_connected_worker_error() {
        use tokio::io::AsyncWriteExt as _;

        let (mut tx, rx) = tokio::io::duplex(4096);
        let resp = WorkerResponse::Error {
            kind: WorkerErrorStage::Connect,
            message: "steam not running".to_owned(),
        };
        tx.write_all(&encode_frame(&resp).unwrap()).await.unwrap();
        drop(tx);

        let result = read_from_duplex(rx).await;
        assert!(matches!(
            result,
            Some(WorkerResponse::Error {
                kind: WorkerErrorStage::Connect,
                ..
            })
        ));
    }
}
