use std::time::Duration;
use tokio::sync::mpsc;

use tokio::sync::mpsc as async_mpsc;

use steamlens_core::ipc::WorkerResponse;

use super::api::{SteamReply, SteamRequest};
use super::dispatch::{error_reply, handle_request, handle_worker_response, reply};
use crate::timeouts;
use crate::worker_subprocess::{WorkerHandle, WorkerMode};

type ConnectError = crate::worker_subprocess::WorkerProtocolError;

async fn await_steam_connected(
    handle: &mut WorkerHandle,
    timeout: Duration,
    reply_sender: &mpsc::UnboundedSender<SteamReply>,
) -> Result<(), ConnectError> {
    match tokio::time::timeout(timeout, handle.recv()).await {
        Ok(Ok(Some(WorkerResponse::SteamConnected { app_name, .. }))) => {
            reply(reply_sender, SteamReply::Connected { app_name });
            Ok(())
        }
        Ok(Ok(Some(WorkerResponse::Error { stage, message }))) => {
            Err(ConnectError::WorkerError { stage, message })
        }
        Ok(Ok(Some(_))) => Err(ConnectError::UnexpectedMessage),
        Ok(Ok(None)) => Err(ConnectError::UnexpectedEof),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(ConnectError::Timeout),
    }
}

pub(super) async fn bridge_loop(
    mut request_receiver: async_mpsc::UnboundedReceiver<SteamRequest>,
    reply_sender: mpsc::UnboundedSender<SteamReply>,
) {
    let connect_timeout = timeouts::STEAM_CONNECT;

    let mut handle = loop {
        let Some(request) = request_receiver.recv().await else {
            return;
        };
        match request {
            SteamRequest::ConnectWithApp(app_id) => {
                match WorkerHandle::spawn(app_id, WorkerMode::Interactive).await {
                    Ok(handle) => break handle,
                    Err(spawn_err) => {
                        reply(
                            &reply_sender,
                            SteamReply::ConnectFailed(spawn_err.to_string()),
                        );
                        continue;
                    }
                }
            }
            SteamRequest::Disconnect => {
                reply(&reply_sender, SteamReply::Disconnected);
                return;
            }
            _ => {
                reply(
                    &reply_sender,
                    SteamReply::RequestStatsFailed("Not connected".to_owned()),
                );
                continue;
            }
        }
    };

    if let Err(e) = await_steam_connected(&mut handle, connect_timeout, &reply_sender).await {
        match e {
            ConnectError::WorkerError { stage, message } => {
                reply(&reply_sender, error_reply(stage, message))
            }
            ConnectError::UnexpectedMessage => reply(
                &reply_sender,
                SteamReply::ConnectFailed("unexpected first message from worker".to_owned()),
            ),
            ConnectError::Timeout => reply(
                &reply_sender,
                SteamReply::ConnectFailed("timed out waiting for SteamConnected".to_owned()),
            ),
            ConnectError::UnexpectedEof | ConnectError::Decode(_) | ConnectError::Write(_) => {
                reply(
                    &reply_sender,
                    SteamReply::ConnectFailed("protocol error".to_owned()),
                )
            }
        }
        let _ = handle.finish().await;
        return;
    }

    loop {
        let request = tokio::select! {
            biased;
            response = handle.recv() => {
                match response {
                    Ok(Some(response)) => {
                        handle_worker_response(response, &reply_sender);
                        continue;
                    }
                    Ok(None) | Err(_) => {
                        let _ = handle.finish().await;
                        return;
                    }
                }
            }
            request = request_receiver.recv() => match request {
                Some(request) => request,
                None => {
                    let _ = handle.finish().await;
                    return;
                }
            },
        };

        match request {
            SteamRequest::ConnectWithApp(new_app_id) => {
                match WorkerHandle::spawn(new_app_id, WorkerMode::Interactive).await {
                    Ok(new_handle) => {
                        let old_handle = std::mem::replace(&mut handle, new_handle);
                        let _ = old_handle.finish().await;

                        if let Err(e) =
                            await_steam_connected(&mut handle, connect_timeout, &reply_sender).await
                        {
                            match e {
                                ConnectError::WorkerError { stage, message } => {
                                    reply(&reply_sender, error_reply(stage, message))
                                }
                                ConnectError::UnexpectedMessage
                                | ConnectError::Timeout
                                | ConnectError::UnexpectedEof
                                | ConnectError::Decode(_)
                                | ConnectError::Write(_) => reply(
                                    &reply_sender,
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
                    Err(spawn_err) => {
                        reply(
                            &reply_sender,
                            SteamReply::ConnectFailed(spawn_err.to_string()),
                        );
                        return;
                    }
                }
            }

            SteamRequest::Disconnect => {
                let _ = handle.finish().await;
                reply(&reply_sender, SteamReply::Disconnected);
                return;
            }

            other => {
                handle_request(other, &mut handle, &reply_sender).await;
            }
        }
    }
}
