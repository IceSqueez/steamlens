use std::collections::HashMap;
use std::io::ErrorKind;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use steamlens_core::ipc::{
    FrameError, WorkerCommand, WorkerErrorKind, WorkerResponse, decode_frame, encode_frame,
    parse_header,
};
use steamlens_core::{
    AchievementData, AchievementIcon, Client, StatKind, StatValue, SteamCallback,
};

use crate::timeouts;

#[derive(Debug)]
enum WorkerError {
    Io(std::io::Error),
    Frame(FrameError),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerError::Io(e) => write!(f, "io: {e}"),
            WorkerError::Frame(e) => write!(f, "frame: {e}"),
        }
    }
}

impl From<std::io::Error> for WorkerError {
    fn from(e: std::io::Error) -> Self {
        WorkerError::Io(e)
    }
}

impl From<FrameError> for WorkerError {
    fn from(e: FrameError) -> Self {
        WorkerError::Frame(e)
    }
}

pub fn run_probe() -> ! {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("probe tokio runtime");

    let exit_code = rt.block_on(probe_main());
    std::process::exit(exit_code);
}

pub fn run(app_id: u32) -> ! {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("worker tokio runtime");

    let exit_code = rt.block_on(worker_main(app_id));
    std::process::exit(exit_code);
}

async fn probe_main() -> i32 {
    let t0 = std::time::Instant::now();
    eprintln!("[probe] connect…");
    let client = match steamlens_core::connect(0) {
        Ok(c) => c,
        Err(e) => {
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                message: e.to_string(),
            })
            .await;
            return 1;
        }
    };
    eprintln!("[probe] connected in {:?}", t0.elapsed());

    let steam_id = client.steam_id();

    let persona_name = match client.persona_name() {
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
    eprintln!("[probe] persona+steamid in {:?}", t0.elapsed());

    let avatar_png = encode_avatar_png(&client);
    eprintln!(
        "[probe] avatar in {:?} ({} bytes)",
        t0.elapsed(),
        avatar_png.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    let t_enum = std::time::Instant::now();
    // `BIsSubscribedApp` filter avoids per-game connect rejections
    // (refunded / expired free-weekend / revoked-license app_ids).
    let games = match client.enumerate_owned_games(true) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[probe] enumerate_owned_games failed: {e}");
            Vec::new()
        }
    };
    eprintln!(
        "[probe] enumerate_owned_games: {} games in {:?} (total {:?})",
        games.len(),
        t_enum.elapsed(),
        t0.elapsed()
    );

    let steam_level = client.get_player_steam_level();
    eprintln!("[probe] steam level: {:?}", steam_level);

    let resp = shm_response_for_probe(steamlens_core::ProbeResultPayload {
        steam_id,
        persona_name,
        avatar_png,
        game_summaries: games,
        steam_level,
    });
    if write_response(&resp).await.is_err() {
        return 1;
    }

    0
}

fn encode_avatar_png(client: &steamlens_core::Client) -> Option<Vec<u8>> {
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    let img = client.user_avatar()?;
    let rgba_image = RgbaImage::from_raw(img.width, img.height, img.rgba)?;
    let mut buf = Cursor::new(Vec::new());
    rgba_image.write_to(&mut buf, ImageFormat::Png).ok()?;
    Some(buf.into_inner())
}

async fn worker_main(app_id: u32) -> i32 {
    let t0 = std::time::Instant::now();
    eprintln!("[worker app_id={app_id}] connect…");
    let client = match steamlens_core::connect(app_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[worker app_id={app_id}] connect failed in {:?}: {e}",
                t0.elapsed()
            );
            let _ = write_response(&WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                message: e.to_string(),
            })
            .await;
            return 1;
        }
    };
    eprintln!("[worker app_id={app_id}] connected in {:?}", t0.elapsed());

    let connected = WorkerResponse::SteamConnected {
        steam_id: client.steam_id(),
        app_name: client.app_name(),
    };
    if write_response(&connected).await.is_err() {
        eprintln!("[worker app_id={app_id}] write SteamConnected failed");
        return 1;
    }

    dispatch_loop(client, app_id).await
}

async fn dispatch_loop(client: Client, app_id: u32) -> i32 {
    let mut stdin = tokio::io::stdin();
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;

            cmd = read_command(&mut stdin) => {
                match cmd {
                    Err(e) => {
                        let _ = write_response(&WorkerResponse::Error {
                            kind: WorkerErrorKind::Generic,
                            message: e.to_string(),
                        }).await;
                        return 1;
                    }
                    Ok(None) => return 0,
                    Ok(Some(command)) => {
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
                    forward_icon_callbacks(callbacks, &client).await;
                }
            }
        }
    }
}

enum DispatchOutcome {
    Continue,
    Shutdown,
    Fatal,
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

        WorkerCommand::ResetAllStats {
            include_achievements,
        } => {
            let resp = reset_all_and_wait(client, include_achievements).await;
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
            let resp = load_achievements_card_only(client).await;
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

async fn load_achievements_and_stats(client: &Client, app_id: u32) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::RequestUserStats,
            message: e.to_string(),
        };
    }

    let received = wait_for_stats_received(client, steam_id).await;
    if let Some(err_resp) = received {
        return err_resp;
    }

    let num = stats_iface.num_achievements();

    if num == 0 {
        return shm_response_for_aas(steamlens_core::AchievementsAndStatsPayload {
            achievements: Vec::new(),
            stats: Vec::new(),
            genre: None,
        });
    }

    let mut achievements = Vec::with_capacity(num as usize);
    for i in 0..num {
        let id = match stats_iface.achievement_name(i) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let display_name = stats_iface
            .achievement_display_attribute(&id, "name")
            .unwrap_or_else(|_| id.clone());
        let description = stats_iface
            .achievement_display_attribute(&id, "desc")
            .unwrap_or_default();
        let hidden_str = stats_iface
            .achievement_display_attribute(&id, "hidden")
            .unwrap_or_default();
        let is_hidden = hidden_str.trim() == "1";
        let (is_achieved, unlock_time) = stats_iface
            .achievement_and_unlock_time(&id)
            .unwrap_or((false, 0));
        let unlock_time = if unlock_time == 0 {
            None
        } else {
            Some(unlock_time)
        };

        let icon = {
            let handle = stats_iface.achievement_icon(&id).unwrap_or(0);
            if handle == 0 {
                None
            } else {
                client
                    .get_image(handle)
                    .ok()
                    .flatten()
                    .map(|img| AchievementIcon {
                        width: img.width,
                        height: img.height,
                        rgba: img.rgba,
                    })
            }
        };

        achievements.push(AchievementData {
            id,
            display_name,
            description,
            is_hidden,
            is_achieved,
            unlock_time,
            permission: 0,
            icon,
        });
    }

    let descriptors = client.stat_descriptors(app_id).unwrap_or_default();
    let mut stats = Vec::with_capacity(descriptors.len());
    for desc in descriptors {
        let value = match desc.kind {
            StatKind::Int => StatValue::Int(stats_iface.get_stat_int(&desc.name).unwrap_or(0)),
            StatKind::Float => {
                StatValue::Float(stats_iface.get_stat_float(&desc.name).unwrap_or(0.0))
            }
        };
        stats.push(steamlens_core::StatData {
            display_name: desc.name.clone(),
            id: desc.name,
            value,
            original_value: value,
            max_value: desc.max_value,
            min_value: desc.min_value,
            default_value: desc.default_value,
            is_increment_only: false,
            permission: 0,
        });
    }

    let genre = client.get_app_data(app_id, c"common/primary_genre");

    shm_response_for_aas(steamlens_core::AchievementsAndStatsPayload {
        achievements,
        stats,
        genre,
    })
}

fn shm_response_for_aas(payload: steamlens_core::AchievementsAndStatsPayload) -> WorkerResponse {
    match steamlens_core::write_payload(&payload) {
        Ok((path, region_bytes)) => WorkerResponse::AchievementsAndStats {
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

fn shm_response_for_count(payload: steamlens_core::AchievementCountPayload) -> WorkerResponse {
    match steamlens_core::write_payload(&payload) {
        Ok((path, region_bytes)) => WorkerResponse::AchievementCount {
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

fn shm_response_for_pct(payload: HashMap<String, f32>) -> WorkerResponse {
    match steamlens_core::write_payload(&payload) {
        Ok((path, region_bytes)) => WorkerResponse::GlobalPercentagesReady {
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

fn shm_response_for_icon(name: String, icon: AchievementIcon) -> WorkerResponse {
    match steamlens_core::write_payload(&icon) {
        Ok((path, region_bytes)) => WorkerResponse::IconUpdated {
            name,
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

fn shm_response_for_probe(payload: steamlens_core::ProbeResultPayload) -> WorkerResponse {
    match steamlens_core::write_payload(&payload) {
        Ok((path, region_bytes)) => WorkerResponse::ProbeResult {
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

fn shm_response_for_card_only(payload: steamlens_core::CardOnlyPayload) -> WorkerResponse {
    match steamlens_core::write_payload(&payload) {
        Ok((path, region_bytes)) => WorkerResponse::CardOnlyAchievements {
            shm_path: path.to_string_lossy().into_owned(),
            region_bytes,
        },
        Err(e) => WorkerResponse::Error {
            kind: WorkerErrorKind::Generic,
            message: e.to_string(),
        },
    }
}

async fn load_achievements_card_only(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::RequestUserStats,
            message: e.to_string(),
        };
    }

    if let Some(early) = wait_for_stats_received_card_only(client, steam_id).await {
        return early;
    }

    let num = stats_iface.num_achievements();

    if num == 0 {
        return shm_response_for_card_only(steamlens_core::CardOnlyPayload {
            achievements: Vec::new(),
        });
    }

    let mut achievements = Vec::with_capacity(num as usize);
    for i in 0..num {
        let id = match stats_iface.achievement_name(i) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let (is_achieved, _) = stats_iface
            .achievement_and_unlock_time(&id)
            .unwrap_or((false, 0));
        achievements.push(steamlens_core::CardOnlyAchievement { id, is_achieved });
    }

    shm_response_for_card_only(steamlens_core::CardOnlyPayload { achievements })
}

async fn quick_achievement_count(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::RequestUserStats,
            message: e.to_string(),
        };
    }

    if let Some(err_resp) = wait_for_stats_received(client, steam_id).await {
        return err_resp;
    }

    let total = stats_iface.num_achievements();

    let mut earned = 0u32;
    for i in 0..total {
        let name = match stats_iface.achievement_name(i) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if let Ok((achieved, _)) = stats_iface.achievement_and_unlock_time(&name)
            && achieved
        {
            earned += 1;
        }
    }

    shm_response_for_count(steamlens_core::AchievementCountPayload { earned, total })
}

async fn wait_for_stats_received(client: &Client, expected_user: u64) -> Option<WorkerResponse> {
    let deadline = std::time::Instant::now() + timeouts::STAT_RECEIVED;
    loop {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsReceived {
                        result,
                        user_steam_id,
                        game_id,
                        ..
                    } = cb
                    {
                        if user_steam_id != expected_user {
                            continue;
                        }
                        eprintln!(
                            "[worker] UserStatsReceived: result={} game={}",
                            result.raw(),
                            game_id,
                        );
                        if result.is_ok() {
                            return None;
                        }
                        if result.raw() == steamlens_core::STEAM_RESULT_NO_STATS_SCHEMA {
                            return Some(shm_response_for_aas(
                                steamlens_core::AchievementsAndStatsPayload {
                                    achievements: Vec::new(),
                                    stats: Vec::new(),
                                    genre: None,
                                },
                            ));
                        }
                        return Some(WorkerResponse::Error {
                            kind: WorkerErrorKind::UserStatsReceived,
                            message: format!("result code {}", result.raw()),
                        });
                    }
                }
            }
            Err(e) => {
                return Some(WorkerResponse::Error {
                    kind: WorkerErrorKind::PollCallbacks,
                    message: e.to_string(),
                });
            }
        }
        if std::time::Instant::now() >= deadline {
            return Some(WorkerResponse::Error {
                kind: WorkerErrorKind::UserStatsReceived,
                message: "timed out waiting for UserStatsReceived".into(),
            });
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

async fn wait_for_stats_received_card_only(
    client: &Client,
    expected_user: u64,
) -> Option<WorkerResponse> {
    let deadline = std::time::Instant::now() + timeouts::STAT_RECEIVED;
    loop {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsReceived {
                        result,
                        user_steam_id,
                        game_id,
                        ..
                    } = cb
                    {
                        if user_steam_id != expected_user {
                            continue;
                        }
                        eprintln!(
                            "[worker] UserStatsReceived (card-only): result={} game={}",
                            result.raw(),
                            game_id,
                        );
                        if result.is_ok() {
                            return None;
                        }
                        if result.raw() == steamlens_core::STEAM_RESULT_NO_STATS_SCHEMA {
                            return Some(shm_response_for_card_only(
                                steamlens_core::CardOnlyPayload {
                                    achievements: Vec::new(),
                                },
                            ));
                        }
                        return Some(WorkerResponse::Error {
                            kind: WorkerErrorKind::UserStatsReceived,
                            message: format!("result code {}", result.raw()),
                        });
                    }
                }
            }
            Err(e) => {
                return Some(WorkerResponse::Error {
                    kind: WorkerErrorKind::PollCallbacks,
                    message: e.to_string(),
                });
            }
        }
        if std::time::Instant::now() >= deadline {
            return Some(WorkerResponse::Error {
                kind: WorkerErrorKind::UserStatsReceived,
                message: "timed out waiting for UserStatsReceived".into(),
            });
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

async fn store_stats_and_wait(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    if let Err(e) = stats_iface.store_stats() {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::StoreStats,
            message: e.to_string(),
        };
    }
    wait_for_store_confirmed(client).await
}

async fn wait_for_store_confirmed(client: &Client) -> WorkerResponse {
    let deadline = std::time::Instant::now() + timeouts::STORE_CONFIRMED;
    loop {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsStored { result, .. } = cb {
                        if result.is_ok() {
                            return WorkerResponse::Stored;
                        } else {
                            return WorkerResponse::Error {
                                kind: WorkerErrorKind::UserStatsStored,
                                message: format!("result code {}", result.raw()),
                            };
                        }
                    }
                }
            }
            Err(e) => {
                return WorkerResponse::Error {
                    kind: WorkerErrorKind::PollCallbacks,
                    message: e.to_string(),
                };
            }
        }
        if std::time::Instant::now() >= deadline {
            return WorkerResponse::Error {
                kind: WorkerErrorKind::StoreStats,
                message: "timed out waiting for UserStatsStored".into(),
            };
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

async fn reset_all_and_wait(client: &Client, include_achievements: bool) -> WorkerResponse {
    let stats_iface = client.user_stats();
    if let Err(e) = stats_iface.reset_all_stats(include_achievements) {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::ResetAllStats,
            message: e.to_string(),
        };
    }
    if let Err(e) = stats_iface.store_stats() {
        return WorkerResponse::Error {
            kind: WorkerErrorKind::StoreStats,
            message: e.to_string(),
        };
    }
    let result = wait_for_store_confirmed(client).await;
    if matches!(result, WorkerResponse::Stored) {
        WorkerResponse::ResetDone
    } else {
        result
    }
}

async fn fetch_global_percentages(client: &Client) -> WorkerResponse {
    use steamlens_core::{CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY, STEAM_RESULT_OK};
    const PAYLOAD_SIZE: usize = 12;

    let handle = match client.user_stats().request_global_achievement_percentages() {
        Ok(h) => h,
        Err(e) => {
            return WorkerResponse::Error {
                kind: WorkerErrorKind::RequestGlobalPercentages,
                message: e.to_string(),
            };
        }
    };

    let deadline = std::time::Instant::now() + timeouts::GLOBAL_PERCENTAGES;
    loop {
        let _ = client.poll_callbacks();

        match client.poll_call_result(
            handle,
            CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY,
            PAYLOAD_SIZE,
        ) {
            Err(e) => {
                return WorkerResponse::Error {
                    kind: WorkerErrorKind::GlobalPercentagesAPICall,
                    message: e.to_string(),
                };
            }
            Ok(None) => {}
            Ok(Some(Err(e))) => {
                return WorkerResponse::Error {
                    kind: WorkerErrorKind::GlobalPercentagesAPICall,
                    message: e.to_string(),
                };
            }
            Ok(Some(Ok(bytes))) => {
                if bytes.len() < PAYLOAD_SIZE {
                    return WorkerResponse::Error {
                        kind: WorkerErrorKind::GlobalPercentagesReady,
                        message: "payload too short".into(),
                    };
                }
                let result_code = i32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0u8; 4]));
                if result_code != STEAM_RESULT_OK {
                    return WorkerResponse::Error {
                        kind: WorkerErrorKind::GlobalPercentagesReady,
                        message: format!("result code {result_code}"),
                    };
                }
                return collect_global_percentages(client);
            }
        }

        if std::time::Instant::now() >= deadline {
            return WorkerResponse::Error {
                kind: WorkerErrorKind::RequestGlobalPercentages,
                message: "timed out waiting for GlobalAchievementPercentagesReady".into(),
            };
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

fn collect_global_percentages(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let num = stats_iface.num_achievements();
    let mut map = HashMap::with_capacity(num as usize);
    for i in 0..num {
        let name = match stats_iface.achievement_name(i) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if let Ok(pct) = stats_iface.achievement_achieved_percent(&name) {
            map.insert(name, pct);
        }
    }
    shm_response_for_pct(map)
}

fn build_icon_response(name: String, img: steamlens_core::Image) -> WorkerResponse {
    shm_response_for_icon(
        name,
        AchievementIcon {
            width: img.width,
            height: img.height,
            rgba: img.rgba,
        },
    )
}

async fn forward_icon_callbacks(callbacks: Vec<SteamCallback>, client: &Client) {
    for cb in callbacks {
        if let SteamCallback::UserAchievementIconFetched {
            achievement_name,
            icon_handle,
            ..
        } = cb
        {
            if icon_handle == 0 {
                continue;
            }
            if let Ok(Some(img)) = client.get_image(icon_handle) {
                let resp = build_icon_response(achievement_name, img);
                let _ = write_response(&resp).await;
            }
        }
    }
}

async fn read_command(
    stdin: &mut (impl AsyncReadExt + Unpin),
) -> Result<Option<WorkerCommand>, WorkerError> {
    let mut header = [0u8; 4];
    match stdin.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(WorkerError::Io(e)),
    }
    let len = parse_header(header)?;
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf).await?;
    let cmd = decode_frame::<WorkerCommand>(&buf)?;
    Ok(Some(cmd))
}

async fn write_response(msg: &WorkerResponse) -> Result<(), WorkerError> {
    let framed = match encode_frame(msg) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[worker] write_response: encode_frame failed: {e}");
            return Err(WorkerError::Frame(e));
        }
    };
    let mut stdout = tokio::io::stdout();
    if let Err(e) = stdout.write_all(&framed).await {
        return Err(WorkerError::Io(e));
    }
    stdout.flush().await?;
    Ok(())
}
