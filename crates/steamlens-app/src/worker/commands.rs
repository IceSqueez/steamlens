use std::collections::HashMap;
use std::time::Instant;

use steamlens_core::ipc::{WorkerErrorStage, WorkerResponse};
use steamlens_core::{AchievementData, AchievementIcon, Client, StatKind, StatValue};

use super::callbacks::{
    poll_and_forward, wait_for_stats_received, wait_for_stats_received_summary,
    wait_for_store_confirmed,
};
use super::shm_responses::{
    shm_response_for_achievements_count, shm_response_for_achievements_full,
    shm_response_for_achievements_summary, shm_response_for_global_percentages,
};
use crate::timeouts;

pub(super) fn encode_avatar_png(client: &Client) -> Option<Vec<u8>> {
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    let raw_avatar = client.user_avatar()?;
    let rgba_image = RgbaImage::from_raw(raw_avatar.width, raw_avatar.height, raw_avatar.rgba)?;
    let mut png_buffer = Cursor::new(Vec::new());
    rgba_image
        .write_to(&mut png_buffer, ImageFormat::Png)
        .ok()?;
    Some(png_buffer.into_inner())
}

pub(super) async fn load_achievements_and_stats(client: &Client, app_id: u32) -> WorkerResponse {
    let user_stats = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = user_stats.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            stage: WorkerErrorStage::RequestUserStats,
            message: e.to_string(),
        };
    }

    let received = wait_for_stats_received(client, steam_id).await;
    if let Some(err_response) = received {
        return err_response;
    }

    let count = user_stats.num_achievements();

    if count == 0 {
        return shm_response_for_achievements_full(steamlens_core::AchievementsFullPayload {
            achievements: Vec::new(),
            stats: Vec::new(),
            genre: None,
        });
    }

    let mut achievements = Vec::with_capacity(count as usize);
    for i in 0..count {
        let id = match user_stats.achievement_name(i) {
            Ok(name) => name,
            Err(_) => continue,
        };
        let display_name = user_stats
            .achievement_display_attribute(&id, "name")
            .unwrap_or_else(|_| id.clone());
        let description = user_stats
            .achievement_display_attribute(&id, "desc")
            .unwrap_or_default();
        let hidden_str = user_stats
            .achievement_display_attribute(&id, "hidden")
            .unwrap_or_default();
        let is_hidden = hidden_str.trim() == "1";
        let (is_achieved, unlock_time) = user_stats
            .achievement_and_unlock_time(&id)
            .unwrap_or((false, 0));
        let unlock_time = if unlock_time == 0 {
            None
        } else {
            Some(unlock_time)
        };

        let icon = {
            let handle = user_stats.achievement_icon(&id).unwrap_or(0);
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
    for descriptor in descriptors {
        let value = match descriptor.kind {
            StatKind::Int => StatValue::Int(user_stats.get_stat_int(&descriptor.name).unwrap_or(0)),
            StatKind::Float => {
                StatValue::Float(user_stats.get_stat_float(&descriptor.name).unwrap_or(0.0))
            }
        };
        let display_name = descriptor
            .display_name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| descriptor.name.clone());
        stats.push(steamlens_core::StatData {
            display_name,
            id: descriptor.name,
            value,
            original_value: value,
            max_value: descriptor.max_value,
            min_value: descriptor.min_value,
            default_value: descriptor.default_value,
            is_increment_only: false,
            permission: 0,
        });
    }

    let genre = client
        .get_app_data(app_id, c"common/primary_genre")
        .and_then(|id| steamlens_core::primary_genre_name(&id).map(str::to_owned));

    shm_response_for_achievements_full(steamlens_core::AchievementsFullPayload {
        achievements,
        stats,
        genre,
    })
}

pub(super) async fn load_achievements_summary(client: &Client, app_id: u32) -> WorkerResponse {
    let user_stats = client.user_stats();
    let steam_id = client.steam_id();
    let genre = client
        .get_app_data(app_id, c"common/primary_genre")
        .and_then(|id| steamlens_core::primary_genre_name(&id).map(str::to_owned));

    let start_time = Instant::now();
    tracing::debug!("request_user_stats start");
    if let Err(e) = user_stats.request_user_stats(steam_id) {
        tracing::error!(
            "request_user_stats failed in {:?}: {e}",
            start_time.elapsed()
        );
        return WorkerResponse::Error {
            stage: WorkerErrorStage::RequestUserStats,
            message: e.to_string(),
        };
    }
    tracing::debug!(
        "request_user_stats sent in {:?}; waiting for UserStatsReceived",
        start_time.elapsed()
    );

    let wait_started = Instant::now();
    if let Some(early) = wait_for_stats_received_summary(client, steam_id).await {
        tracing::debug!(
            "stats wait returned early (likely error/no-schema) in {:?}",
            wait_started.elapsed()
        );
        return early;
    }
    tracing::info!("UserStatsReceived OK in {:?}", wait_started.elapsed());

    let count = user_stats.num_achievements();

    if count == 0 {
        return shm_response_for_achievements_summary(steamlens_core::AchievementsSummaryPayload {
            achievements: Vec::new(),
            genre,
        });
    }

    let mut achievements = Vec::with_capacity(count as usize);
    for i in 0..count {
        let id = match user_stats.achievement_name(i) {
            Ok(name) => name,
            Err(_) => continue,
        };
        let (is_achieved, _) = user_stats
            .achievement_and_unlock_time(&id)
            .unwrap_or((false, 0));
        achievements.push(steamlens_core::AchievementSummary { id, is_achieved });
    }

    shm_response_for_achievements_summary(steamlens_core::AchievementsSummaryPayload {
        achievements,
        genre,
    })
}

pub(super) async fn load_achievement_count(client: &Client) -> WorkerResponse {
    let user_stats = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = user_stats.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            stage: WorkerErrorStage::RequestUserStats,
            message: e.to_string(),
        };
    }

    if let Some(err_response) = wait_for_stats_received(client, steam_id).await {
        return err_response;
    }

    let total = user_stats.num_achievements();

    let mut earned = 0u32;
    for i in 0..total {
        let name = match user_stats.achievement_name(i) {
            Ok(name) => name,
            Err(_) => continue,
        };
        if let Ok((achieved, _)) = user_stats.achievement_and_unlock_time(&name)
            && achieved
        {
            earned += 1;
        }
    }

    shm_response_for_achievements_count(steamlens_core::AchievementsCountPayload { earned, total })
}

pub(super) async fn store_stats_and_wait(client: &Client) -> WorkerResponse {
    let user_stats = client.user_stats();
    if let Err(e) = user_stats.store_stats() {
        return WorkerResponse::Error {
            stage: WorkerErrorStage::StoreStats,
            message: e.to_string(),
        };
    }
    wait_for_store_confirmed(client).await
}

pub(super) async fn fetch_global_percentages(client: &Client) -> WorkerResponse {
    use steamlens_core::{CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY, STEAM_RESULT_OK};
    const PAYLOAD_SIZE: usize = 12;

    let handle = match client.user_stats().request_global_achievement_percentages() {
        Ok(handle) => handle,
        Err(e) => {
            return WorkerResponse::Error {
                stage: WorkerErrorStage::RequestGlobalPercentages,
                message: e.to_string(),
            };
        }
    };

    let deadline = Instant::now() + timeouts::GLOBAL_PERCENTAGES;
    loop {
        poll_and_forward(client).await;

        match client.poll_call_result(
            handle,
            CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY,
            PAYLOAD_SIZE,
        ) {
            Err(e) => {
                return WorkerResponse::Error {
                    stage: WorkerErrorStage::GlobalPercentagesApiCall,
                    message: e.to_string(),
                };
            }
            Ok(None) => {}
            Ok(Some(Err(e))) => {
                return WorkerResponse::Error {
                    stage: WorkerErrorStage::GlobalPercentagesApiCall,
                    message: e.to_string(),
                };
            }
            Ok(Some(Ok(bytes))) => {
                poll_and_forward(client).await;
                if bytes.len() < PAYLOAD_SIZE {
                    return WorkerResponse::Error {
                        stage: WorkerErrorStage::GlobalPercentagesReady,
                        message: "payload too short".into(),
                    };
                }
                let result_code = i32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0u8; 4]));
                if result_code != STEAM_RESULT_OK {
                    return WorkerResponse::Error {
                        stage: WorkerErrorStage::GlobalPercentagesReady,
                        message: format!("result code {result_code}"),
                    };
                }
                return collect_global_percentages(client);
            }
        }

        if Instant::now() >= deadline {
            return WorkerResponse::Error {
                stage: WorkerErrorStage::RequestGlobalPercentages,
                message: "timed out waiting for GlobalAchievementPercentagesReady".into(),
            };
        }
        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }
}

fn collect_global_percentages(client: &Client) -> WorkerResponse {
    let user_stats = client.user_stats();
    let count = user_stats.num_achievements();
    let mut percentages = HashMap::with_capacity(count as usize);
    for i in 0..count {
        let name = match user_stats.achievement_name(i) {
            Ok(name) => name,
            Err(_) => continue,
        };
        if let Ok(percent) = user_stats.achievement_achieved_percent(&name) {
            percentages.insert(name, percent);
        }
    }
    shm_response_for_global_percentages(percentages)
}
