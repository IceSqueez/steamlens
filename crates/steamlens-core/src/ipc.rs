pub mod shm;
pub mod types;

use thiserror::Error;

pub use types::{
    AchievementCountPayload, AchievementData, AchievementIcon, AchievementsAndStatsPayload,
    CardOnlyAchievement, CardOnlyPayload, ProbeResultPayload, StatData, StatValue,
};

pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WorkerErrorKind {
    Connect,
    NotLoggedIn,
    RequestUserStats,
    UserStatsReceived,
    PollCallbacks,
    StoreStats,
    UserStatsStored,
    ResetAllStats,
    RequestGlobalPercentages,
    GlobalPercentagesReady,
    GlobalPercentagesAPICall,
    Generic,
}

impl WorkerErrorKind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::NotLoggedIn => "not_logged_in",
            Self::RequestUserStats => "request_user_stats",
            Self::UserStatsReceived => "user_stats_received",
            Self::PollCallbacks => "poll_callbacks",
            Self::StoreStats => "store_stats",
            Self::UserStatsStored => "user_stats_stored",
            Self::ResetAllStats => "reset_all_stats",
            Self::RequestGlobalPercentages => "request_global_percentages",
            Self::GlobalPercentagesReady => "global_percentages_ready",
            Self::GlobalPercentagesAPICall => "global_percentages_api_call",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("frame too large: {size} bytes (max {max})")]
    TooLarge { size: usize, max: usize },
    #[error("encode failed: {0}")]
    Encode(String),
    #[error("decode failed: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkerCommand {
    LoadAchievementsAndStats,
    SetAchievement(String),
    ClearAchievement(String),
    SetStatInt { name: String, value: i32 },
    SetStatFloat { name: String, value: f32 },
    StoreStats,
    ResetAllStats { include_achievements: bool },
    RequestGlobalPercentages,
    QuickAchievementCount,
    Shutdown,
    LoadAchievementsAndStatsCardOnly,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkerResponse {
    SteamConnected {
        steam_id: u64,
        app_name: Option<String>,
    },
    Ack,
    AchievementsAndStats {
        shm_path: String,
        region_bytes: u64,
    },
    IconUpdated {
        name: String,
        shm_path: String,
        region_bytes: u64,
    },
    GlobalPercentagesReady {
        shm_path: String,
        region_bytes: u64,
    },
    Stored,
    ResetDone,
    AchievementCount {
        shm_path: String,
        region_bytes: u64,
    },
    ProbeResult {
        shm_path: String,
        region_bytes: u64,
    },
    Error {
        kind: WorkerErrorKind,
        message: String,
    },
    Disconnected,
    CardOnlyAchievements {
        shm_path: String,
        region_bytes: u64,
    },
}

pub fn frame_header(payload_len: usize) -> Result<[u8; 4], FrameError> {
    if payload_len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            size: payload_len,
            max: MAX_FRAME_BYTES,
        });
    }
    let len_u32 = payload_len as u32;
    Ok(len_u32.to_le_bytes())
}

pub fn parse_header(bytes: [u8; 4]) -> Result<usize, FrameError> {
    let len = u32::from_le_bytes(bytes) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            size: len,
            max: MAX_FRAME_BYTES,
        });
    }
    Ok(len)
}

pub fn encode_frame<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let payload = postcard::to_allocvec(msg).map_err(|e| FrameError::Encode(e.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            size: payload.len(),
            max: MAX_FRAME_BYTES,
        });
    }
    let header = frame_header(payload.len())?;
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_frame<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, FrameError> {
    postcard::from_bytes(bytes).map_err(|e| FrameError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{AchievementData, StatValue};

    fn all_commands() -> Vec<WorkerCommand> {
        vec![
            WorkerCommand::LoadAchievementsAndStats,
            WorkerCommand::SetAchievement("ACH_WIN".to_owned()),
            WorkerCommand::ClearAchievement("ACH_LOSE".to_owned()),
            WorkerCommand::SetStatInt {
                name: "stat_kills".to_owned(),
                value: 42,
            },
            WorkerCommand::SetStatFloat {
                name: "stat_ratio".to_owned(),
                value: std::f32::consts::PI,
            },
            WorkerCommand::StoreStats,
            WorkerCommand::ResetAllStats {
                include_achievements: true,
            },
            WorkerCommand::RequestGlobalPercentages,
            WorkerCommand::QuickAchievementCount,
            WorkerCommand::Shutdown,
            WorkerCommand::LoadAchievementsAndStatsCardOnly,
        ]
    }

    fn all_responses() -> Vec<WorkerResponse> {
        vec![
            WorkerResponse::SteamConnected {
                steam_id: 76561198000000000,
                app_name: Some("Terraria".to_owned()),
            },
            WorkerResponse::SteamConnected {
                steam_id: 1,
                app_name: None,
            },
            WorkerResponse::Ack,
            WorkerResponse::AchievementsAndStats {
                shm_path: "/dev/shm/steamlens-test-aas-XYZ".to_owned(),
                region_bytes: 8192,
            },
            WorkerResponse::IconUpdated {
                name: "ACH_FOO".to_owned(),
                shm_path: "/dev/shm/steamlens-test-icon-XYZ".to_owned(),
                region_bytes: 262_144,
            },
            WorkerResponse::GlobalPercentagesReady {
                shm_path: "/dev/shm/steamlens-test-pct-XYZ".to_owned(),
                region_bytes: 1024,
            },
            WorkerResponse::Stored,
            WorkerResponse::ResetDone,
            WorkerResponse::AchievementCount {
                shm_path: "/dev/shm/steamlens-test-count-XYZ".to_owned(),
                region_bytes: 16,
            },
            WorkerResponse::ProbeResult {
                shm_path: "/dev/shm/steamlens-test-probe-XYZ".to_owned(),
                region_bytes: 4096,
            },
            WorkerResponse::Error {
                kind: WorkerErrorKind::StoreStats,
                message: "pipe closed".to_owned(),
            },
            WorkerResponse::Disconnected,
            WorkerResponse::CardOnlyAchievements {
                shm_path: "/dev/shm/steamlens-test-card-XYZ".to_owned(),
                region_bytes: 512,
            },
        ]
    }

    fn all_error_kinds() -> Vec<WorkerErrorKind> {
        vec![
            WorkerErrorKind::Connect,
            WorkerErrorKind::NotLoggedIn,
            WorkerErrorKind::RequestUserStats,
            WorkerErrorKind::UserStatsReceived,
            WorkerErrorKind::PollCallbacks,
            WorkerErrorKind::StoreStats,
            WorkerErrorKind::UserStatsStored,
            WorkerErrorKind::ResetAllStats,
            WorkerErrorKind::RequestGlobalPercentages,
            WorkerErrorKind::GlobalPercentagesReady,
            WorkerErrorKind::GlobalPercentagesAPICall,
            WorkerErrorKind::Generic,
        ]
    }

    #[test]
    fn worker_error_kind_tags_are_unique_and_nonempty() {
        let kinds = all_error_kinds();
        let tags: Vec<&str> = kinds.iter().map(|k| k.tag()).collect();
        for tag in &tags {
            assert!(!tag.is_empty(), "tag must not be empty");
        }
        let mut deduped = tags.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(
            deduped.len(),
            tags.len(),
            "all tags must be unique: {tags:?}"
        );
    }

    #[test]
    fn worker_error_kind_roundtrip_via_response_frame() {
        for kind in all_error_kinds() {
            let resp = WorkerResponse::Error {
                kind,
                message: format!("test error for {:?}", kind),
            };
            let framed = encode_frame(&resp).expect("encode must succeed");
            let payload = &framed[4..];
            let decoded: WorkerResponse = decode_frame(payload).expect("decode must succeed");
            match decoded {
                WorkerResponse::Error {
                    kind: decoded_kind,
                    message: decoded_msg,
                } => {
                    assert_eq!(decoded_kind, kind, "kind must round-trip: {:?}", kind);
                    assert!(
                        decoded_msg.contains(kind.tag()) || !decoded_msg.is_empty(),
                        "message must be non-empty"
                    );
                }
                other => panic!(
                    "expected WorkerResponse::Error, got {:?}",
                    std::mem::discriminant(&other)
                ),
            }
        }
    }

    #[test]
    fn encode_then_decode_roundtrip_command() {
        for cmd in all_commands() {
            let framed = encode_frame(&cmd).expect("encode must succeed");
            assert!(framed.len() >= 4, "must have at least 4 header bytes");
            let payload = &framed[4..];
            let decoded: WorkerCommand = decode_frame(payload).expect("decode must succeed");
            let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
            assert_eq!(
                framed,
                re_framed,
                "round-trip must be stable: {:?}",
                std::mem::discriminant(&cmd)
            );
        }
    }

    #[test]
    fn encode_then_decode_roundtrip_response() {
        for resp in all_responses() {
            let framed = encode_frame(&resp).expect("encode must succeed");
            assert!(framed.len() >= 4);
            let payload = &framed[4..];
            let decoded: WorkerResponse = decode_frame(payload).expect("decode must succeed");
            let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
            assert_eq!(
                framed,
                re_framed,
                "round-trip must be stable: {:?}",
                std::mem::discriminant(&resp)
            );
        }
    }

    #[test]
    fn frame_header_roundtrip() {
        for &size in &[0usize, 1, 128, 4096, MAX_FRAME_BYTES] {
            let header = frame_header(size).expect("must succeed for valid size");
            let parsed = parse_header(header).expect("parse must succeed");
            assert_eq!(parsed, size, "parse(frame_header(size)) == size");
        }
    }

    #[test]
    fn frame_header_rejects_oversize() {
        let result = frame_header(MAX_FRAME_BYTES + 1);
        assert!(
            matches!(result, Err(FrameError::TooLarge { .. })),
            "must reject payload > MAX_FRAME_BYTES"
        );
    }

    #[test]
    fn parse_header_rejects_oversize() {
        let oversize_len = (MAX_FRAME_BYTES + 1) as u32;
        let bytes = oversize_len.to_le_bytes();
        let result = parse_header(bytes);
        assert!(
            matches!(result, Err(FrameError::TooLarge { .. })),
            "must reject incoming length > MAX_FRAME_BYTES"
        );
    }

    #[test]
    fn decode_frame_rejects_garbage() {
        let garbage = b"not valid bincode at all, just random bytes 12345";
        let result: Result<WorkerCommand, _> = decode_frame(garbage);
        assert!(
            result.is_err(),
            "must return Err, not panic, on garbage input"
        );
        assert!(
            matches!(result, Err(FrameError::Decode(_))),
            "error must be FrameError::Decode"
        );
    }

    #[test]
    fn serde_field_compatibility_basic() {
        let original = AchievementData {
            id: "ACH_TEST".to_owned(),
            display_name: "Test Achievement".to_owned(),
            description: "Do something.".to_owned(),
            is_hidden: true,
            is_achieved: false,
            unlock_time: None,
            permission: 3,
            icon: None,
        };

        let framed = encode_frame(&original).expect("encode must succeed");
        let payload = &framed[4..];
        let decoded: AchievementData = decode_frame(payload).expect("decode must succeed");

        assert_eq!(decoded.id, original.id);
        assert_eq!(decoded.display_name, original.display_name);
        assert_eq!(decoded.description, original.description);
        assert_eq!(decoded.is_hidden, original.is_hidden);
        assert_eq!(decoded.is_achieved, original.is_achieved);
        assert_eq!(decoded.unlock_time, original.unlock_time);
        assert_eq!(decoded.permission, original.permission);
        assert!(decoded.icon.is_none());
    }

    #[test]
    fn encode_frame_header_bytes_match_payload_length() {
        let cmd = WorkerCommand::SetAchievement("ACH_WIN".to_owned());
        let framed = encode_frame(&cmd).expect("encode must succeed");
        let reported_len = parse_header(framed[..4].try_into().unwrap()).unwrap();
        let actual_payload_len = framed.len() - 4;
        assert_eq!(
            reported_len, actual_payload_len,
            "header must report exact payload byte count"
        );
    }

    #[test]
    fn stat_value_int_roundtrip() {
        let v = StatValue::Int(-1024);
        let framed = encode_frame(&v).unwrap();
        let decoded: StatValue = decode_frame(&framed[4..]).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn ack_roundtrip() {
        let framed = encode_frame(&WorkerResponse::Ack).expect("encode must succeed");
        assert!(framed.len() >= 4, "framed must contain at least a header");
        let payload = &framed[4..];
        let decoded: WorkerResponse = decode_frame(payload).expect("decode must succeed");
        assert!(
            matches!(decoded, WorkerResponse::Ack),
            "decoded variant must be Ack"
        );
        let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
        assert_eq!(framed, re_framed, "Ack round-trip must be byte-stable");
    }

    #[test]
    fn quick_achievement_count_command_roundtrip() {
        let cmd = WorkerCommand::QuickAchievementCount;
        let framed = encode_frame(&cmd).expect("encode must succeed");
        let payload = &framed[4..];
        let decoded: WorkerCommand = decode_frame(payload).expect("decode must succeed");
        let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
        assert_eq!(framed, re_framed, "QuickAchievementCount must round-trip");
    }

    #[test]
    fn stat_value_float_roundtrip() {
        let v = StatValue::Float(std::f32::consts::PI);
        let framed = encode_frame(&v).unwrap();
        let decoded: StatValue = decode_frame(&framed[4..]).unwrap();
        assert!(
            matches!(decoded, StatValue::Float(f) if (f - std::f32::consts::PI).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn card_only_command_roundtrip() {
        let cmd = WorkerCommand::LoadAchievementsAndStatsCardOnly;
        let framed = encode_frame(&cmd).expect("encode must succeed");
        let payload = &framed[4..];
        let decoded: WorkerCommand = decode_frame(payload).expect("decode must succeed");
        let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
        assert_eq!(
            framed, re_framed,
            "LoadAchievementsAndStatsCardOnly must round-trip byte-stable"
        );
    }

    #[test]
    fn card_only_response_roundtrip() {
        let resp = WorkerResponse::CardOnlyAchievements {
            shm_path: "/dev/shm/steamlens-card-abc123".to_owned(),
            region_bytes: 1024,
        };
        let framed = encode_frame(&resp).expect("encode must succeed");
        let payload = &framed[4..];
        let decoded: WorkerResponse = decode_frame(payload).expect("decode must succeed");
        let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
        assert_eq!(
            framed, re_framed,
            "CardOnlyAchievements response must round-trip byte-stable"
        );
        match decoded {
            WorkerResponse::CardOnlyAchievements {
                shm_path,
                region_bytes,
            } => {
                assert_eq!(shm_path, "/dev/shm/steamlens-card-abc123");
                assert_eq!(region_bytes, 1024);
            }
            _ => panic!("decoded variant must be CardOnlyAchievements"),
        }
    }

    #[test]
    fn card_only_payload_roundtrip_with_achievements() {
        use types::CardOnlyAchievement;

        let payload = types::CardOnlyPayload {
            achievements: vec![
                CardOnlyAchievement {
                    id: "ACH_FIRST_KILL".to_owned(),
                    is_achieved: true,
                },
                CardOnlyAchievement {
                    id: "ACH_HUNDRED".to_owned(),
                    is_achieved: false,
                },
            ],
            genre: Some("Action".to_owned()),
        };

        let bytes = postcard::to_allocvec(&payload).expect("serialize");
        let restored: types::CardOnlyPayload = postcard::from_bytes(&bytes).expect("decode");

        assert_eq!(restored.achievements.len(), 2);
        assert_eq!(restored.achievements[0].id, "ACH_FIRST_KILL");
        assert!(restored.achievements[0].is_achieved);
        assert_eq!(restored.achievements[1].id, "ACH_HUNDRED");
        assert!(!restored.achievements[1].is_achieved);
        assert_eq!(restored.genre.as_deref(), Some("Action"));
    }

    #[test]
    fn card_only_payload_roundtrip_empty() {
        let payload = types::CardOnlyPayload {
            achievements: Vec::new(),
            genre: None,
        };
        let bytes = postcard::to_allocvec(&payload).expect("serialize");
        let restored: types::CardOnlyPayload = postcard::from_bytes(&bytes).expect("decode");
        assert!(restored.achievements.is_empty());
        assert!(restored.genre.is_none());
    }

    #[test]
    fn card_only_achievement_field_equality() {
        use types::CardOnlyAchievement;
        let a = CardOnlyAchievement {
            id: "X".to_owned(),
            is_achieved: true,
        };
        let b = CardOnlyAchievement {
            id: "X".to_owned(),
            is_achieved: true,
        };
        let c = CardOnlyAchievement {
            id: "X".to_owned(),
            is_achieved: false,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn achievement_count_payload_roundtrip() {
        for (earned, total) in [(0u32, 0u32), (12, 30), (u32::MAX, u32::MAX)] {
            let p = AchievementCountPayload { earned, total };
            let bytes = postcard::to_allocvec(&p).expect("serialize");
            let restored: AchievementCountPayload = postcard::from_bytes(&bytes).expect("decode");
            assert_eq!(restored.earned, earned);
            assert_eq!(restored.total, total);
        }
    }

    #[test]
    fn probe_result_payload_roundtrip_with_avatar() {
        let avatar_bytes = vec![137u8, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13];
        let p = ProbeResultPayload {
            steam_id: 76561198000000042,
            persona_name: "TestUser".to_owned(),
            avatar_png: Some(avatar_bytes.clone()),
            game_summaries: vec![crate::library::GameSummary {
                app_id: 12345,
                change_number: 0,
                last_played: Some(1_700_000_000),
            }],
            steam_level: Some(42),
        };
        let bytes = postcard::to_allocvec(&p).expect("serialize");
        let restored: ProbeResultPayload = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(restored.steam_id, 76561198000000042);
        assert_eq!(restored.persona_name, "TestUser");
        assert_eq!(
            restored.avatar_png.as_deref(),
            Some(avatar_bytes.as_slice())
        );
        assert_eq!(restored.game_summaries.len(), 1);
        assert_eq!(restored.game_summaries[0].app_id, 12345);
        assert_eq!(restored.steam_level, Some(42));
    }

    #[test]
    fn probe_result_payload_roundtrip_no_avatar() {
        let p = ProbeResultPayload {
            steam_id: 1,
            persona_name: "Ghost".to_owned(),
            avatar_png: None,
            game_summaries: vec![],
            steam_level: None,
        };
        let bytes = postcard::to_allocvec(&p).expect("serialize");
        let restored: ProbeResultPayload = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(restored.steam_id, 1);
        assert_eq!(restored.persona_name, "Ghost");
        assert!(restored.avatar_png.is_none());
        assert!(restored.game_summaries.is_empty());
        assert!(restored.steam_level.is_none());
    }
}
