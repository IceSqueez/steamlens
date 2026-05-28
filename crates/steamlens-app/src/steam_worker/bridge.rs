use std::time::Duration;
use tokio::sync::mpsc;

use tokio::sync::mpsc as async_mpsc;

use steamlens_core::ipc::WorkerResponse;

use super::api::{SteamReply, SteamRequest};
use super::dispatch::{error_reply, handle_request, handle_worker_response, reply};
use crate::timeouts;
use crate::worker_subprocess::{WorkerHandle, WorkerMode, WorkerSpawnError};

type ConnectError = crate::worker_subprocess::WorkerProtocolError;

async fn await_steam_connected(
    handle: &mut WorkerHandle,
    timeout: Duration,
    rep_tx: &mpsc::UnboundedSender<SteamReply>,
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

fn spawn_error_message(e: WorkerSpawnError) -> String {
    e.to_string()
}

pub(super) async fn bridge_loop(
    mut req_rx: async_mpsc::UnboundedReceiver<SteamRequest>,
    rep_tx: mpsc::UnboundedSender<SteamReply>,
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
