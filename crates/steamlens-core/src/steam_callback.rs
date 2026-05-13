use crate::raw_callback::RawCallback;

pub const CALLBACK_ID_USER_STATS_RECEIVED: i32 = 1101;
pub const CALLBACK_ID_USER_STATS_STORED: i32 = 1102;
pub const CALLBACK_ID_USER_ACHIEVEMENT_ICON_FETCHED: i32 = 1109;
pub const CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY: i32 = 1110;

pub const STEAM_RESULT_OK: i32 = 1;
pub const STEAM_RESULT_NO_STATS_SCHEMA: i32 = 2;

/// Only `Ok` (raw `1`) is named; all other Steam result codes are
/// preserved as `Other(i32)` for caller inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamResult {
    Ok,
    Other(i32),
}

impl SteamResult {
    pub fn from_raw(code: i32) -> Self {
        if code == 1 {
            Self::Ok
        } else {
            Self::Other(code)
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn raw(&self) -> i32 {
        match self {
            Self::Ok => 1,
            Self::Other(c) => *c,
        }
    }
}

/// Unknown or malformed payloads fall through to `Unknown(RawCallback)`.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SteamCallback {
    UserStatsReceived {
        game_id: u64,
        result: SteamResult,
        user_steam_id: u64,
    },
    UserStatsStored {
        game_id: u64,
        result: SteamResult,
    },
    UserAchievementIconFetched {
        game_id: u64,
        achievement_name: String,
        achieved: bool,
        icon_handle: i32,
    },
    GlobalAchievementPercentagesReady {
        game_id: u64,
        result: SteamResult,
    },
    Unknown(RawCallback),
}

impl From<RawCallback> for SteamCallback {
    fn from(raw: RawCallback) -> Self {
        callback_decode::decode(raw)
    }
}

pub(crate) mod callback_decode {
    use super::{
        CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY,
        CALLBACK_ID_USER_ACHIEVEMENT_ICON_FETCHED, CALLBACK_ID_USER_STATS_RECEIVED,
        CALLBACK_ID_USER_STATS_STORED, SteamCallback, SteamResult,
    };
    use crate::raw_callback::RawCallback;

    const USER_STATS_RECEIVED_LEN: usize = 20;
    const USER_STATS_STORED_LEN: usize = 12;
    const USER_ACHIEVEMENT_ICON_FETCHED_LEN: usize = 144;
    const GLOBAL_ACHIEVEMENT_PERCENTAGES_READY_LEN: usize = 12;

    pub(crate) fn decode_user_stats_received(payload: &[u8]) -> Option<SteamCallback> {
        if payload.len() < USER_STATS_RECEIVED_LEN {
            return None;
        }
        let game_id = u64::from_le_bytes(payload[0..8].try_into().ok()?);
        let result = i32::from_le_bytes(payload[8..12].try_into().ok()?);
        let user_steam_id = u64::from_le_bytes(payload[12..20].try_into().ok()?);
        Some(SteamCallback::UserStatsReceived {
            game_id,
            result: SteamResult::from_raw(result),
            user_steam_id,
        })
    }

    pub(crate) fn decode_user_stats_stored(payload: &[u8]) -> Option<SteamCallback> {
        if payload.len() < USER_STATS_STORED_LEN {
            return None;
        }
        let game_id = u64::from_le_bytes(payload[0..8].try_into().ok()?);
        let result = i32::from_le_bytes(payload[8..12].try_into().ok()?);
        Some(SteamCallback::UserStatsStored {
            game_id,
            result: SteamResult::from_raw(result),
        })
    }

    pub(crate) fn decode_user_achievement_icon_fetched(payload: &[u8]) -> Option<SteamCallback> {
        if payload.len() < USER_ACHIEVEMENT_ICON_FETCHED_LEN {
            return None;
        }
        let game_id = u64::from_le_bytes(payload[0..8].try_into().ok()?);
        let name_bytes = &payload[8..136];
        let nul_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(128);
        let achievement_name = std::str::from_utf8(&name_bytes[..nul_pos]).ok()?.to_owned();
        let achieved = payload[136] != 0;
        let icon_handle = i32::from_le_bytes(payload[140..144].try_into().ok()?);
        Some(SteamCallback::UserAchievementIconFetched {
            game_id,
            achievement_name,
            achieved,
            icon_handle,
        })
    }

    pub(crate) fn decode_global_percentages_ready(payload: &[u8]) -> Option<SteamCallback> {
        if payload.len() < GLOBAL_ACHIEVEMENT_PERCENTAGES_READY_LEN {
            return None;
        }
        let game_id = u64::from_le_bytes(payload[0..8].try_into().ok()?);
        let result = i32::from_le_bytes(payload[8..12].try_into().ok()?);
        Some(SteamCallback::GlobalAchievementPercentagesReady {
            game_id,
            result: SteamResult::from_raw(result),
        })
    }

    pub(crate) fn decode(raw: RawCallback) -> SteamCallback {
        match raw.id {
            CALLBACK_ID_USER_STATS_RECEIVED => {
                decode_user_stats_received(&raw.payload).unwrap_or(SteamCallback::Unknown(raw))
            }
            CALLBACK_ID_USER_STATS_STORED => {
                decode_user_stats_stored(&raw.payload).unwrap_or(SteamCallback::Unknown(raw))
            }
            CALLBACK_ID_USER_ACHIEVEMENT_ICON_FETCHED => {
                decode_user_achievement_icon_fetched(&raw.payload)
                    .unwrap_or(SteamCallback::Unknown(raw))
            }
            CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY => {
                decode_global_percentages_ready(&raw.payload).unwrap_or(SteamCallback::Unknown(raw))
            }
            _ => SteamCallback::Unknown(raw),
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::raw_callback::RawCallback;

        fn make_icon_fetched_payload(
            game_id: u64,
            name: &[u8; 128],
            achieved: bool,
            icon_handle: i32,
        ) -> Vec<u8> {
            let mut payload = Vec::with_capacity(144);
            payload.extend_from_slice(&game_id.to_le_bytes());
            payload.extend_from_slice(name.as_slice());
            payload.push(if achieved { 1u8 } else { 0u8 });
            payload.extend_from_slice(&[0u8; 3]);
            payload.extend_from_slice(&icon_handle.to_le_bytes());
            payload
        }

        fn name_buf(s: &str) -> [u8; 128] {
            let mut buf = [0u8; 128];
            let bytes = s.as_bytes();
            let len = bytes.len().min(128);
            buf[..len].copy_from_slice(&bytes[..len]);
            buf
        }

        #[test]
        fn decode_icon_fetched_achieved() {
            let payload = make_icon_fetched_payload(105600, &name_buf("ACH_FOO"), true, 42);
            let raw = RawCallback { id: 1109, payload };
            let cb = crate::SteamCallback::from(raw);
            match cb {
                crate::SteamCallback::UserAchievementIconFetched {
                    game_id,
                    achievement_name,
                    achieved,
                    icon_handle,
                } => {
                    assert_eq!(game_id, 105600);
                    assert_eq!(achievement_name, "ACH_FOO");
                    assert!(achieved);
                    assert_eq!(icon_handle, 42);
                }
                other => panic!("expected UserAchievementIconFetched, got {other:?}"),
            }
        }

        #[test]
        fn decode_icon_fetched_not_achieved() {
            let payload = make_icon_fetched_payload(105600, &name_buf("ACH_BAR"), false, 7);
            let raw = RawCallback { id: 1109, payload };
            let cb = crate::SteamCallback::from(raw);
            match cb {
                crate::SteamCallback::UserAchievementIconFetched {
                    achieved,
                    icon_handle,
                    ..
                } => {
                    assert!(!achieved);
                    assert_eq!(icon_handle, 7);
                }
                other => panic!("expected UserAchievementIconFetched, got {other:?}"),
            }
        }

        #[test]
        fn decode_icon_fetched_name_fills_128_bytes_no_nul() {
            let mut name = [b'X'; 128];
            name[0] = b'A';
            let payload = make_icon_fetched_payload(105600, &name, true, 99);
            let raw = RawCallback { id: 1109, payload };
            let cb = crate::SteamCallback::from(raw);
            match cb {
                crate::SteamCallback::UserAchievementIconFetched {
                    achievement_name, ..
                } => {
                    assert_eq!(achievement_name.len(), 128);
                    assert!(achievement_name.starts_with('A'));
                }
                other => panic!("expected UserAchievementIconFetched, got {other:?}"),
            }
        }

        #[test]
        fn decode_icon_fetched_truncated_returns_unknown_via_public_decode() {
            let payload = vec![0u8; 143];
            let raw = RawCallback { id: 1109, payload };
            let cb = crate::SteamCallback::from(raw);
            assert!(
                matches!(cb, crate::SteamCallback::Unknown(_)),
                "truncated icon-fetched payload must fall back to Unknown"
            );
        }

        #[test]
        fn decode_global_percentages_ok() {
            let game_id: u64 = 105600;
            let result_ok: i32 = 1;
            let mut payload = Vec::with_capacity(12);
            payload.extend_from_slice(&game_id.to_le_bytes());
            payload.extend_from_slice(&result_ok.to_le_bytes());
            let raw = RawCallback { id: 1110, payload };
            let cb = crate::SteamCallback::from(raw);
            match cb {
                crate::SteamCallback::GlobalAchievementPercentagesReady {
                    game_id: gid,
                    result,
                } => {
                    assert_eq!(gid, 105600);
                    assert!(result.is_ok());
                }
                other => panic!("expected GlobalAchievementPercentagesReady, got {other:?}"),
            }
        }

        #[test]
        fn decode_global_percentages_too_short_returns_unknown() {
            let payload = vec![0u8; 8];
            let raw = RawCallback { id: 1110, payload };
            let cb = crate::SteamCallback::from(raw);
            assert!(
                matches!(cb, crate::SteamCallback::Unknown(_)),
                "8-byte payload (too short) must fall back to Unknown"
            );
        }
    }
}
