use std::time::Duration;
use tokio::sync::mpsc;

use tokio::sync::mpsc as async_mpsc;

use steamlens_core::ipc::WorkerResponse;

use super::api::{SteamReply, SteamRequest, WorkerReply};
use super::dispatch::{error_reply, handle_request, handle_worker_response, reply};
use crate::timeouts;
use crate::worker_subprocess::{WorkerHandle, WorkerMode};

type ConnectError = crate::worker_subprocess::WorkerProtocolError;

async fn await_steam_connected(
    handle: &mut WorkerHandle,
    timeout: Duration,
    reply_sender: &mpsc::UnboundedSender<WorkerReply>,
    app_id: u32,
) -> Result<(), ConnectError> {
    match tokio::time::timeout(timeout, handle.recv()).await {
        Ok(Ok(Some(WorkerResponse::SteamConnected { app_name, .. }))) => {
            reply(reply_sender, app_id, SteamReply::Connected { app_name });
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
    reply_sender: mpsc::UnboundedSender<WorkerReply>,
    generation: u64,
) {
    let connect_timeout = timeouts::STEAM_CONNECT;
    tracing::trace!(generation, "bridge_loop started");

    let (mut handle, mut current_app_id) = loop {
        let Some(request) = request_receiver.recv().await else {
            tracing::trace!(
                generation,
                "bridge_loop exit: channel closed before any connect"
            );
            return;
        };
        match request {
            SteamRequest::ConnectWithApp(app_id) => {
                match WorkerHandle::spawn(app_id, WorkerMode::Interactive).await {
                    Ok(handle) => break (handle, app_id),
                    Err(spawn_err) => {
                        reply(
                            &reply_sender,
                            app_id,
                            SteamReply::ConnectFailed(spawn_err.to_string()),
                        );
                        continue;
                    }
                }
            }
            _ => {
                continue;
            }
        }
    };

    if let Err(e) =
        await_steam_connected(&mut handle, connect_timeout, &reply_sender, current_app_id).await
    {
        match e {
            ConnectError::WorkerError { stage, message } => {
                reply(&reply_sender, current_app_id, error_reply(stage, message))
            }
            ConnectError::UnexpectedMessage => reply(
                &reply_sender,
                current_app_id,
                SteamReply::ConnectFailed("unexpected first message from worker".to_owned()),
            ),
            ConnectError::Timeout => reply(
                &reply_sender,
                current_app_id,
                SteamReply::ConnectFailed("timed out waiting for SteamConnected".to_owned()),
            ),
            ConnectError::UnexpectedEof | ConnectError::Decode(_) | ConnectError::Write(_) => {
                reply(
                    &reply_sender,
                    current_app_id,
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
            request = request_receiver.recv() => match request {
                Some(request) => request,
                None => {
                    tracing::trace!(generation, app_id = current_app_id, "bridge_loop exit: request channel closed");
                    let _ = handle.finish().await;
                    return;
                }
            },
            response = handle.recv() => {
                match response {
                    Ok(Some(response)) => {
                        handle_worker_response(response, &reply_sender, current_app_id);
                        continue;
                    }
                    Ok(None) | Err(_) => {
                        tracing::trace!(generation, app_id = current_app_id, "bridge_loop exit: subprocess pipe closed");
                        let _ = handle.finish().await;
                        return;
                    }
                }
            }
        };

        match request {
            SteamRequest::ConnectWithApp(new_app_id) => {
                match WorkerHandle::spawn(new_app_id, WorkerMode::Interactive).await {
                    Ok(new_handle) => {
                        let old_handle = std::mem::replace(&mut handle, new_handle);
                        let _ = old_handle.finish().await;
                        current_app_id = new_app_id;

                        if let Err(e) = await_steam_connected(
                            &mut handle,
                            connect_timeout,
                            &reply_sender,
                            current_app_id,
                        )
                        .await
                        {
                            match e {
                                ConnectError::WorkerError { stage, message } => reply(
                                    &reply_sender,
                                    current_app_id,
                                    error_reply(stage, message),
                                ),
                                ConnectError::UnexpectedMessage
                                | ConnectError::Timeout
                                | ConnectError::UnexpectedEof
                                | ConnectError::Decode(_)
                                | ConnectError::Write(_) => reply(
                                    &reply_sender,
                                    current_app_id,
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
                            new_app_id,
                            SteamReply::ConnectFailed(spawn_err.to_string()),
                        );
                        return;
                    }
                }
            }

            other => {
                handle_request(other, &mut handle, &reply_sender, current_app_id).await;
            }
        }
    }
}
