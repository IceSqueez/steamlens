mod callbacks;
mod commands;
mod dispatch;
mod error;
mod ipc_io;
mod shm_responses;

use std::process;
use std::time::Instant;

use steamlens_core::ipc::{WorkerErrorStage, WorkerResponse};

use commands::encode_avatar_png;
use dispatch::dispatch_loop;
use error::error_chain;
use ipc_io::write_response;
use shm_responses::shm_response_for_probe;

pub fn run_probe() -> ! {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("probe tokio runtime");

    let exit_code = runtime.block_on(probe_main());
    process::exit(exit_code);
}

pub fn run(app_id: u32) -> ! {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("worker tokio runtime");

    let exit_code = runtime.block_on(worker_main(app_id));
    process::exit(exit_code);
}

async fn probe_main() -> i32 {
    let start_time = Instant::now();
    let candidates: Vec<String> = steamlens_core::steamclient_lib_candidates()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    tracing::info!(?candidates, "probe: steamclient.dll discovery candidates");
    let client = match steamlens_core::connect(0) {
        Ok(client) => client,
        Err(steamlens_core::SteamError::NotLoggedIn) => {
            tracing::warn!("probe: Steam is running but no user is signed in");
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::NotLoggedIn,
                message: steamlens_core::SteamError::NotLoggedIn.to_string(),
            })
            .await;
            return 1;
        }
        Err(e @ steamlens_core::SteamError::GlobalUserUnavailable { .. }) => {
            tracing::warn!("probe: {}", error_chain(&e));
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::GlobalUserUnavailable,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
        Err(e) => {
            tracing::error!("probe: connect failed: {}", error_chain(&e));
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::Connect,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
    };
    tracing::info!("probe: connected in {:?}", start_time.elapsed());

    let steam_id = client.steam_id();

    let nickname = match client.nickname() {
        Some(name) => name,
        None => {
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::Generic,
                message: "GetPersonaName returned null or empty".into(),
            })
            .await;
            return 1;
        }
    };
    tracing::debug!("probe: persona+steamid in {:?}", start_time.elapsed());

    let avatar_png = encode_avatar_png(&client);
    tracing::debug!(
        "probe: avatar in {:?} ({} bytes)",
        start_time.elapsed(),
        avatar_png.as_ref().map(|bytes| bytes.len()).unwrap_or(0)
    );

    let enumerate_started = Instant::now();
    let games = match client.enumerate_owned_games(true) {
        Ok(games) => games,
        Err(e) => {
            tracing::warn!("probe: enumerate_owned_games failed: {e}");
            Vec::new()
        }
    };
    tracing::info!(
        "probe: enumerate_owned_games: {} games in {:?} (total {:?})",
        games.len(),
        enumerate_started.elapsed(),
        start_time.elapsed()
    );

    let steam_level = client.get_player_steam_level();
    tracing::debug!("probe: steam level: {:?}", steam_level);

    let steam_root = client.steam_root().ok();
    tracing::debug!("probe: steam_root: {:?}", steam_root);

    let response = shm_response_for_probe(steamlens_core::ProbeResultPayload {
        steam_id,
        nickname,
        avatar_png,
        game_summaries: games,
        steam_level,
        steam_root,
    });
    if write_response(&response).await.is_err() {
        return 1;
    }

    0
}

async fn worker_main(app_id: u32) -> i32 {
    let start_time = Instant::now();
    tracing::debug!("connect…");
    let client = match steamlens_core::connect(app_id) {
        Ok(client) => client,
        Err(e @ steamlens_core::SteamError::GlobalUserUnavailable { .. }) => {
            tracing::warn!("{} (after {:?})", error_chain(&e), start_time.elapsed());
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::GlobalUserUnavailable,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
        Err(e) => {
            tracing::error!(
                "connect failed in {:?}: {}",
                start_time.elapsed(),
                error_chain(&e)
            );
            let _ = write_response(&WorkerResponse::Error {
                stage: WorkerErrorStage::Connect,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
    };
    tracing::info!("connected in {:?}", start_time.elapsed());

    let connected = WorkerResponse::SteamConnected {
        steam_id: client.steam_id(),
        app_name: client.app_name(),
    };
    if write_response(&connected).await.is_err() {
        tracing::error!("write SteamConnected failed");
        return 1;
    }

    dispatch_loop(client, app_id).await
}
