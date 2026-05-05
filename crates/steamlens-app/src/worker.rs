use std::collections::HashMap;
use std::io::ErrorKind;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use steamlens_core::ipc::{
    FrameError, WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};
use steamlens_core::{
    AchievementData, AchievementIcon, Client, StatKind, StatValue, SteamCallback,
};

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
    let client = match steamlens_core::connect(0) {
        Ok(c) => c,
        Err(e) => {
            let _ = write_response(&WorkerResponse::Error {
                context: "probe".into(),
                message: e.to_string(),
            })
            .await;
            return 1;
        }
    };

    let steam_id = client.steam_id();

    let persona_name = match client.persona_name() {
        Some(n) => n,
        None => {
            let _ = write_response(&WorkerResponse::Error {
                context: "probe".into(),
                message: "GetPersonaName returned null or empty".into(),
            })
            .await;
            return 1;
        }
    };

    let avatar_png = encode_avatar_png(&client);

    let resp = WorkerResponse::ProbeResult {
        steam_id,
        persona_name,
        avatar_png,
    };
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
    let client = match steamlens_core::connect(app_id) {
        Ok(c) => c,
        Err(e) => {
            let _ = write_response(&WorkerResponse::Error {
                context: "connect".into(),
                message: e.to_string(),
            })
            .await;
            return 1;
        }
    };

    let hello = WorkerResponse::Hello {
        steam_id: client.steam_id(),
        app_name: client.app_name(),
    };
    if write_response(&hello).await.is_err() {
        return 1;
    }

    dispatch_loop(client).await
}

async fn dispatch_loop(client: Client) -> i32 {
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
                            context: "read_command".into(),
                            message: e.to_string(),
                        }).await;
                        return 1;
                    }
                    Ok(None) => return 0,
                    Ok(Some(command)) => {
                        match handle_command(command, &client).await {
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

async fn handle_command(cmd: WorkerCommand, client: &Client) -> DispatchOutcome {
    match cmd {
        WorkerCommand::Hello => {
            let resp = WorkerResponse::Hello {
                steam_id: client.steam_id(),
                app_name: client.app_name(),
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::LoadAchievementsAndStats => {
            let resp = load_achievements_and_stats(client);
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
            let pct = fetch_global_percentages(client);
            if write_response(&pct).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::SetAchievement(name) => {
            let resp = match client.user_stats().set_achievement(&name) {
                Ok(()) => WorkerResponse::Ack,
                Err(e) => WorkerResponse::Error {
                    context: format!("SetAchievement({name})"),
                    message: e.to_string(),
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
                    context: format!("ClearAchievement({name})"),
                    message: e.to_string(),
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
                    context: format!("SetStatInt({name})"),
                    message: e.to_string(),
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
                    context: format!("SetStatFloat({name})"),
                    message: e.to_string(),
                },
            };
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::StoreStats => {
            let resp = store_stats_and_wait(client);
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::ResetAllStats {
            include_achievements,
        } => {
            let resp = reset_all_and_wait(client, include_achievements);
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::RequestGlobalPercentages => {
            let resp = fetch_global_percentages(client);
            if write_response(&resp).await.is_err() {
                return DispatchOutcome::Fatal;
            }
        }

        WorkerCommand::QuickAchievementCount => {
            let resp = quick_achievement_count(client);
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

fn load_achievements_and_stats(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            context: "RequestUserStats".into(),
            message: e.to_string(),
        };
    }

    let received = wait_for_stats_received(client, steam_id);
    if let Some(err_resp) = received {
        return err_resp;
    }

    let num = match stats_iface.num_achievements() {
        Ok(n) => n,
        Err(e) => {
            return WorkerResponse::Error {
                context: "num_achievements".into(),
                message: e.to_string(),
            };
        }
    };

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

        let handle = stats_iface.achievement_icon(&id).unwrap_or(0);
        let icon = if handle == 0 {
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

    let app_id = client.app_id();
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

    WorkerResponse::AchievementsAndStats {
        achievements,
        stats,
    }
}

fn quick_achievement_count(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            context: "QuickAchievementCount/RequestUserStats".into(),
            message: e.to_string(),
        };
    }

    if let Some(err_resp) = wait_for_stats_received(client, steam_id) {
        return err_resp;
    }

    let total = match stats_iface.num_achievements() {
        Ok(n) => n,
        Err(e) => {
            return WorkerResponse::Error {
                context: "QuickAchievementCount/num_achievements".into(),
                message: e.to_string(),
            };
        }
    };

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

    WorkerResponse::AchievementCount { earned, total }
}

fn wait_for_stats_received(client: &Client, expected_user: u64) -> Option<WorkerResponse> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsReceived {
                        result,
                        user_steam_id,
                        ..
                    } = cb
                        && user_steam_id == expected_user
                    {
                        if result.is_ok() {
                            return None;
                        } else {
                            return Some(WorkerResponse::Error {
                                context: "UserStatsReceived".into(),
                                message: format!("result code {}", result.raw()),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                return Some(WorkerResponse::Error {
                    context: "poll_callbacks".into(),
                    message: e.to_string(),
                });
            }
        }
        if std::time::Instant::now() >= deadline {
            return Some(WorkerResponse::Error {
                context: "UserStatsReceived".into(),
                message: "timed out waiting for UserStatsReceived".into(),
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn store_stats_and_wait(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    if let Err(e) = stats_iface.store_stats() {
        return WorkerResponse::Error {
            context: "StoreStats".into(),
            message: e.to_string(),
        };
    }
    wait_for_store_confirmed(client)
}

fn wait_for_store_confirmed(client: &Client) -> WorkerResponse {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsStored { result, .. } = cb {
                        if result.is_ok() {
                            return WorkerResponse::Stored;
                        } else {
                            return WorkerResponse::Error {
                                context: "UserStatsStored".into(),
                                message: format!("result code {}", result.raw()),
                            };
                        }
                    }
                }
            }
            Err(e) => {
                return WorkerResponse::Error {
                    context: "poll_callbacks".into(),
                    message: e.to_string(),
                };
            }
        }
        if std::time::Instant::now() >= deadline {
            return WorkerResponse::Error {
                context: "StoreStats".into(),
                message: "timed out waiting for UserStatsStored".into(),
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn reset_all_and_wait(client: &Client, include_achievements: bool) -> WorkerResponse {
    let stats_iface = client.user_stats();
    if let Err(e) = stats_iface.reset_all_stats(include_achievements) {
        return WorkerResponse::Error {
            context: "ResetAllStats".into(),
            message: e.to_string(),
        };
    }
    if let Err(e) = stats_iface.store_stats() {
        return WorkerResponse::Error {
            context: "StoreStats after reset".into(),
            message: e.to_string(),
        };
    }
    let result = wait_for_store_confirmed(client);
    if matches!(result, WorkerResponse::Stored) {
        WorkerResponse::ResetDone
    } else {
        result
    }
}

fn fetch_global_percentages(client: &Client) -> WorkerResponse {
    const CALLBACK_ID_GLOBAL: i32 = 1110;
    const PAYLOAD_SIZE: usize = 12;

    let handle = match client.user_stats().request_global_achievement_percentages() {
        Ok(h) => h,
        Err(e) => {
            return WorkerResponse::Error {
                context: "RequestGlobalAchievementPercentages".into(),
                message: e.to_string(),
            };
        }
    };

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        // Drain broadcast callbacks to prevent Steam's internal queue from stalling.
        // Icons arriving here are dropped — the parent will see them when it next
        // polls for icon updates via the async dispatch loop.
        let _ = client.poll_callbacks();

        match client.poll_call_result(handle, CALLBACK_ID_GLOBAL, PAYLOAD_SIZE) {
            Err(e) => {
                return WorkerResponse::Error {
                    context: "poll_call_result".into(),
                    message: e.to_string(),
                };
            }
            Ok(None) => {}
            Ok(Some(Err(e))) => {
                return WorkerResponse::Error {
                    context: "GlobalAchievementPercentages APICall".into(),
                    message: e.to_string(),
                };
            }
            Ok(Some(Ok(bytes))) => {
                if bytes.len() < PAYLOAD_SIZE {
                    return WorkerResponse::Error {
                        context: "GlobalAchievementPercentagesReady".into(),
                        message: "payload too short".into(),
                    };
                }
                let result_code = i32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0u8; 4]));
                if result_code != 1 {
                    return WorkerResponse::Error {
                        context: "GlobalAchievementPercentagesReady".into(),
                        message: format!("result code {result_code}"),
                    };
                }
                return collect_global_percentages(client);
            }
        }

        if std::time::Instant::now() >= deadline {
            return WorkerResponse::Error {
                context: "RequestGlobalPercentages".into(),
                message: "timed out waiting for GlobalAchievementPercentagesReady".into(),
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn collect_global_percentages(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let num = match stats_iface.num_achievements() {
        Ok(n) => n,
        Err(e) => {
            return WorkerResponse::Error {
                context: "num_achievements (percentages)".into(),
                message: e.to_string(),
            };
        }
    };
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
    WorkerResponse::GlobalPercentagesReady(map)
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
                let icon = AchievementIcon {
                    width: img.width,
                    height: img.height,
                    rgba: img.rgba,
                };
                let resp = WorkerResponse::IconUpdated {
                    name: achievement_name,
                    icon,
                };
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
    let framed = encode_frame(msg)?;
    let mut stdout = tokio::io::stdout();
    stdout.write_all(&framed).await?;
    stdout.flush().await?;
    Ok(())
}
