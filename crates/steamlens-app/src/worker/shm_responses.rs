use std::collections::HashMap;

use steamlens_core::AchievementIcon;
use steamlens_core::ipc::{WorkerErrorKind, WorkerResponse};

pub(super) fn shm_response_for_aas(
    payload: steamlens_core::AchievementsAndStatsPayload,
) -> WorkerResponse {
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

pub(super) fn shm_response_for_count(
    payload: steamlens_core::AchievementCountPayload,
) -> WorkerResponse {
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

pub(super) fn shm_response_for_pct(payload: HashMap<String, f32>) -> WorkerResponse {
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

pub(super) fn shm_response_for_icon(name: String, icon: AchievementIcon) -> WorkerResponse {
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

pub(super) fn shm_response_for_probe(
    payload: steamlens_core::ProbeResultPayload,
) -> WorkerResponse {
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

pub(super) fn shm_response_for_card_only(
    payload: steamlens_core::CardOnlyPayload,
) -> WorkerResponse {
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

pub(super) fn build_icon_response(name: String, img: steamlens_core::Image) -> WorkerResponse {
    shm_response_for_icon(
        name,
        AchievementIcon {
            width: img.width,
            height: img.height,
            rgba: img.rgba,
        },
    )
}
