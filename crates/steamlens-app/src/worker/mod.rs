mod callbacks;
mod commands;
mod dispatch;
mod error;
mod ipc_io;
mod shm_responses;

use std::process;
use std::time::Instant;

use steamlens_core::ipc::{WorkerErrorKind, WorkerResponse};

use commands::encode_avatar_png;
use dispatch::dispatch_loop;
use error::error_chain;
use ipc_io::write_response;
use shm_responses::shm_response_for_probe;

pub fn run_probe() -> ! {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("probe tokio runtime");

    let exit_code = rt.block_on(probe_main());
    process::exit(exit_code);
}

pub fn run(app_id: u32) -> ! {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("worker tokio runtime");

    let exit_code = rt.block_on(worker_main(app_id));
    process::exit(exit_code);
}

async fn probe_main() -> i32 {
    let t0 = Instant::now();
    let candidates: Vec<String> = steamlens_core::steamclient_lib_candidates()
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();
    tracing::info!(?candidates, "probe: steamclient.dll discovery candidates");
    let client = match steamlens_core::connect(0) {
        Ok(c) => c,
        Err(steamlens_core::SteamError::NotLoggedIn) => {
            tracing::warn!("probe: Steam is running but no user is signed in");
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::NotLoggedIn,
                message: steamlens_core::SteamError::NotLoggedIn.to_string(),
            })
            .await;
            return 1;
        }
        Err(e) => {
            tracing::error!("probe: connect failed: {}", error_chain(&e));
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
    };
    tracing::info!("probe: connected in {:?}", t0.elapsed());

    let steam_id = client.steam_id();

    let nickname = match client.nickname() {
        Some(n) => n,
        None => {
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::Generic,
                message: "GetPersonaName returned null or empty".into(),
            })
            .await;
            return 1;
        }
    };
    tracing::debug!("probe: persona+steamid in {:?}", t0.elapsed());

    let avatar_png = encode_avatar_png(&client);
    tracing::debug!(
        "probe: avatar in {:?} ({} bytes)",
        t0.elapsed(),
        avatar_png.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    let t_enum = Instant::now();
    let games = match client.enumerate_owned_games(true) {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("probe: enumerate_owned_games failed: {e}");
            Vec::new()
        }
    };
    tracing::info!(
        "probe: enumerate_owned_games: {} games in {:?} (total {:?})",
        games.len(),
        t_enum.elapsed(),
        t0.elapsed()
    );

    let steam_level = client.get_player_steam_level();
    tracing::debug!("probe: steam level: {:?}", steam_level);

    let steam_root = client.steam_root().ok();
    tracing::debug!("probe: steam_root: {:?}", steam_root);

    let resp = shm_response_for_probe(steamlens_core::ProbeResultPayload {
        steam_id,
        nickname,
        avatar_png,
        game_summaries: games,
        steam_level,
        steam_root,
    });
    if write_response(&resp).await.is_err() {
        return 1;
    }

    0
}

async fn worker_main(app_id: u32) -> i32 {
    let t0 = Instant::now();
    tracing::debug!("connect…");
    let client = match steamlens_core::connect(app_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("connect failed in {:?}: {}", t0.elapsed(), error_chain(&e));
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                message: error_chain(&e),
            })
            .await;
            return 1;
        }
    };
    tracing::info!("connected in {:?}", t0.elapsed());

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
