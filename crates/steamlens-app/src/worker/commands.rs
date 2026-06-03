use std::collections::HashMap;
use std::time::Instant;

use steamlens_core::ipc::{WorkerErrorStage, WorkerResponse};
use steamlens_core::{AchievementData, AchievementIcon, Client, StatKind, StatValue};

use super::callbacks::{
    poll_and_forward, wait_for_stats_received, wait_for_stats_received_card_only,
    wait_for_store_confirmed,
};
use super::shm_responses::{
    shm_response_for_aas, shm_response_for_card_only, shm_response_for_count, shm_response_for_pct,
};
use crate::timeouts;

pub(super) fn encode_avatar_png(client: &Client) -> Option<Vec<u8>> {
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    let img = client.user_avatar()?;
    let rgba_image = RgbaImage::from_raw(img.width, img.height, img.rgba)?;
    let mut buf = Cursor::new(Vec::new());
    rgba_image.write_to(&mut buf, ImageFormat::Png).ok()?;
    Some(buf.into_inner())
}

pub(super) async fn load_achievements_and_stats(client: &Client, app_id: u32) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            kind: WorkerErrorStage::RequestUserStats,
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
        let display_name = desc
            .display_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| desc.name.clone());
        stats.push(steamlens_core::StatData {
            display_name,
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

    let genre = client
        .get_app_data(app_id, c"common/primary_genre")
        .and_then(|id| steamlens_core::primary_genre_name(&id).map(str::to_owned));

    shm_response_for_aas(steamlens_core::AchievementsAndStatsPayload {
        achievements,
        stats,
        genre,
    })
}

pub(super) async fn load_achievements_card_only(client: &Client, app_id: u32) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();
    let genre = client
        .get_app_data(app_id, c"common/primary_genre")
        .and_then(|id| steamlens_core::primary_genre_name(&id).map(str::to_owned));

    let t0 = Instant::now();
    tracing::debug!("request_user_stats start");
    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        tracing::error!("request_user_stats failed in {:?}: {e}", t0.elapsed());
        return WorkerResponse::Error {
            kind: WorkerErrorStage::RequestUserStats,
            message: e.to_string(),
        };
    }
    tracing::debug!(
        "request_user_stats sent in {:?}; waiting for UserStatsReceived",
        t0.elapsed()
    );

    let t_wait = Instant::now();
    if let Some(early) = wait_for_stats_received_card_only(client, steam_id).await {
        tracing::debug!(
            "stats wait returned early (likely error/no-schema) in {:?}",
            t_wait.elapsed()
        );
        return early;
    }
    tracing::info!("UserStatsReceived OK in {:?}", t_wait.elapsed());

    let num = stats_iface.num_achievements();

    if num == 0 {
        return shm_response_for_card_only(steamlens_core::CardOnlyPayload {
            achievements: Vec::new(),
            genre,
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

    shm_response_for_card_only(steamlens_core::CardOnlyPayload {
        achievements,
        genre,
    })
}

pub(super) async fn quick_achievement_count(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    let steam_id = client.steam_id();

    if let Err(e) = stats_iface.request_user_stats(steam_id) {
        return WorkerResponse::Error {
            kind: WorkerErrorStage::RequestUserStats,
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

pub(super) async fn store_stats_and_wait(client: &Client) -> WorkerResponse {
    let stats_iface = client.user_stats();
    if let Err(e) = stats_iface.store_stats() {
        return WorkerResponse::Error {
            kind: WorkerErrorStage::StoreStats,
            message: e.to_string(),
        };
    }
    wait_for_store_confirmed(client).await
}

pub(super) async fn fetch_global_percentages(client: &Client) -> WorkerResponse {
    use steamlens_core::{CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY, STEAM_RESULT_OK};
    const PAYLOAD_SIZE: usize = 12;

    let handle = match client.user_stats().request_global_achievement_percentages() {
        Ok(h) => h,
        Err(e) => {
            return WorkerResponse::Error {
                kind: WorkerErrorStage::RequestGlobalPercentages,
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
                    kind: WorkerErrorStage::GlobalPercentagesAPICall,
                    message: e.to_string(),
                };
            }
            Ok(None) => {}
            Ok(Some(Err(e))) => {
                return WorkerResponse::Error {
                    kind: WorkerErrorStage::GlobalPercentagesAPICall,
                    message: e.to_string(),
                };
            }
            Ok(Some(Ok(bytes))) => {
                poll_and_forward(client).await;
                if bytes.len() < PAYLOAD_SIZE {
                    return WorkerResponse::Error {
                        kind: WorkerErrorStage::GlobalPercentagesReady,
                        message: "payload too short".into(),
                    };
                }
                let result_code = i32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0u8; 4]));
                if result_code != STEAM_RESULT_OK {
                    return WorkerResponse::Error {
                        kind: WorkerErrorStage::GlobalPercentagesReady,
                        message: format!("result code {result_code}"),
                    };
                }
                return collect_global_percentages(client);
            }
        }

        if Instant::now() >= deadline {
            return WorkerResponse::Error {
                kind: WorkerErrorStage::RequestGlobalPercentages,
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
