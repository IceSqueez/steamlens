pub mod types;

use thiserror::Error;

pub use types::{AchievementData, AchievementIcon, StatData, StatValue};

pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

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
    Hello,
    LoadAchievementsAndStats,
    SetAchievement(String),
    ClearAchievement(String),
    SetStatInt { name: String, value: i32 },
    SetStatFloat { name: String, value: f32 },
    StoreStats,
    ResetAllStats { include_achievements: bool },
    RequestGlobalPercentages,
    Shutdown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkerResponse {
    Hello {
        steam_id: u64,
        app_name: Option<String>,
    },
    AchievementsAndStats {
        achievements: Vec<AchievementData>,
        stats: Vec<StatData>,
    },
    IconUpdated {
        name: String,
        icon: AchievementIcon,
    },
    GlobalPercentagesReady(std::collections::HashMap<String, f32>),
    Stored,
    ResetDone,
    Error {
        context: String,
        message: String,
    },
    Disconnected,
}

/// Returns the 4-byte LE length header for a payload of `payload_len` bytes.
///
/// The caller appends the payload bytes immediately after this header. The
/// combined wire format is therefore `[u32 LE length][payload bytes]`.
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

/// Parses the 4-byte LE length header written by [`frame_header`].
///
/// Returns the payload byte count. Returns [`FrameError::TooLarge`] if the
/// encoded length exceeds [`MAX_FRAME_BYTES`].
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

/// Serialises `msg` with bincode and prepends the 4-byte LE length header.
///
/// Returns the complete framed bytes (header + payload). The caller writes
/// these bytes verbatim to the transport.
pub fn encode_frame<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let payload = bincode::serialize(msg).map_err(|e| FrameError::Encode(e.to_string()))?;
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

/// Deserialises `bytes` (the payload slice, **without** the length header) into `T`.
///
/// Returns [`FrameError::Decode`] on any bincode error including trailing bytes.
pub fn decode_frame<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, FrameError> {
    bincode::deserialize(bytes).map_err(|e| FrameError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use types::{AchievementData, AchievementIcon, StatData, StatValue};

    fn all_commands() -> Vec<WorkerCommand> {
        vec![
            WorkerCommand::Hello,
            WorkerCommand::LoadAchievementsAndStats,
            WorkerCommand::SetAchievement("ACH_WIN".to_owned()),
            WorkerCommand::ClearAchievement("ACH_LOSE".to_owned()),
            WorkerCommand::SetStatInt {
                name: "stat_kills".to_owned(),
                value: 42,
            },
            WorkerCommand::SetStatFloat {
                name: "stat_ratio".to_owned(),
                value: 3.14,
            },
            WorkerCommand::StoreStats,
            WorkerCommand::ResetAllStats {
                include_achievements: true,
            },
            WorkerCommand::RequestGlobalPercentages,
            WorkerCommand::Shutdown,
        ]
    }

    fn all_responses() -> Vec<WorkerResponse> {
        let mut pct_map = HashMap::new();
        pct_map.insert("ACH_EASY".to_owned(), 95.5f32);
        pct_map.insert("ACH_HARD".to_owned(), 0.3f32);

        vec![
            WorkerResponse::Hello {
                steam_id: 76561198000000000,
                app_name: Some("Terraria".to_owned()),
            },
            WorkerResponse::Hello {
                steam_id: 1,
                app_name: None,
            },
            WorkerResponse::AchievementsAndStats {
                achievements: vec![AchievementData {
                    id: "ACH_1".to_owned(),
                    display_name: "First!".to_owned(),
                    description: "Do the thing.".to_owned(),
                    is_hidden: false,
                    is_achieved: true,
                    unlock_time: Some(1_700_000_000),
                    permission: 0,
                    icon: Some(AchievementIcon {
                        width: 2,
                        height: 2,
                        rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255],
                    }),
                }],
                stats: vec![StatData {
                    id: "kills".to_owned(),
                    display_name: "Kills".to_owned(),
                    value: StatValue::Int(99),
                    original_value: StatValue::Int(99),
                    max_value: Some(1000),
                    min_value: Some(0),
                    default_value: Some(0),
                    is_increment_only: true,
                    permission: 0,
                }],
            },
            WorkerResponse::IconUpdated {
                name: "ACH_FOO".to_owned(),
                icon: AchievementIcon {
                    width: 1,
                    height: 1,
                    rgba: vec![128, 128, 128, 255],
                },
            },
            WorkerResponse::GlobalPercentagesReady(pct_map),
            WorkerResponse::Stored,
            WorkerResponse::ResetDone,
            WorkerResponse::Error {
                context: "StoreStats".to_owned(),
                message: "pipe closed".to_owned(),
            },
            WorkerResponse::Disconnected,
        ]
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

            match (&resp, &decoded) {
                (
                    WorkerResponse::GlobalPercentagesReady(orig_map),
                    WorkerResponse::GlobalPercentagesReady(dec_map),
                ) => {
                    assert_eq!(orig_map.len(), dec_map.len(), "map sizes must match");
                    for (k, v) in orig_map {
                        let got = dec_map
                            .get(k)
                            .unwrap_or_else(|| panic!("key {k} missing after roundtrip"));
                        assert!(
                            (got - v).abs() < f32::EPSILON,
                            "value mismatch for key {k}: {v} != {got}"
                        );
                    }
                }
                _ => {
                    let re_framed = encode_frame(&decoded).expect("re-encode must succeed");
                    assert_eq!(
                        framed,
                        re_framed,
                        "round-trip must be stable: {:?}",
                        std::mem::discriminant(&resp)
                    );
                }
            }
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
    fn stat_value_float_roundtrip() {
        let v = StatValue::Float(std::f32::consts::PI);
        let framed = encode_frame(&v).unwrap();
        let decoded: StatValue = decode_frame(&framed[4..]).unwrap();
        assert!(
            matches!(decoded, StatValue::Float(f) if (f - std::f32::consts::PI).abs() < f32::EPSILON)
        );
    }
}
