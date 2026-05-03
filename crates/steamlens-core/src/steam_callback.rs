use crate::raw_callback::RawCallback;

pub const CALLBACK_ID_USER_STATS_RECEIVED: i32 = 1101;
pub const CALLBACK_ID_USER_STATS_STORED: i32 = 1102;

/// A Steam EResult value.
///
/// Carries the raw integer code returned in callback payloads. Only `k_EResultOK`
/// (code 1) is given a named variant; all other codes are preserved as `Other(i32)`
/// so callers can inspect them without this crate needing to enumerate all 100+
/// Steamworks result codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamResult {
    /// `k_EResultOK = 1` — the operation succeeded.
    Ok,
    /// Any result code other than 1. The wrapped value is the raw `EResult` integer.
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

/// A typed Steam callback received from the callback poller.
///
/// Obtain values via [`crate::Client::poll_callbacks`]. Poll at ~10 Hz from an
/// `iced::Subscription` on the UI thread.
///
/// Variants are produced by decoding raw callback payloads from Steam. Unknown
/// or malformed payloads fall back to the `Unknown` variant — the raw bytes are
/// preserved so callers can inspect or log them without data loss.
#[derive(Debug, Clone)]
pub enum SteamCallback {
    /// Steam has delivered the requested stats and achievements for a user.
    ///
    /// Emitted in response to a `RequestUserStats` call. Until this arrives with
    /// `result.is_ok()`, `get_stat_*` and `get_achievement` methods return Steam
    /// defaults (0 / `false`).
    UserStatsReceived {
        game_id: u64,
        result: SteamResult,
        user_steam_id: u64,
    },

    /// Steam has persisted staged stat changes for a user.
    ///
    /// Emitted after a successful `StoreStats` call.
    UserStatsStored { game_id: u64, result: SteamResult },

    /// An unrecognised or malformed callback. The raw id and payload bytes are
    /// preserved for inspection and logging.
    Unknown(RawCallback),
}

impl From<RawCallback> for SteamCallback {
    fn from(raw: RawCallback) -> Self {
        callback_decode::decode(raw)
    }
}

pub(crate) mod callback_decode {
    use super::{
        CALLBACK_ID_USER_STATS_RECEIVED, CALLBACK_ID_USER_STATS_STORED, SteamCallback, SteamResult,
    };
    use crate::raw_callback::RawCallback;

    const USER_STATS_RECEIVED_LEN: usize = 20;
    const USER_STATS_STORED_LEN: usize = 12;

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

    pub(crate) fn decode(raw: RawCallback) -> SteamCallback {
        match raw.id {
            CALLBACK_ID_USER_STATS_RECEIVED => {
                decode_user_stats_received(&raw.payload).unwrap_or(SteamCallback::Unknown(raw))
            }
            CALLBACK_ID_USER_STATS_STORED => {
                decode_user_stats_stored(&raw.payload).unwrap_or(SteamCallback::Unknown(raw))
            }
            _ => SteamCallback::Unknown(raw),
        }
    }
}
