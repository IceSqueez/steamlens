use steamlens_core::{RawCallback, SteamCallback, SteamResult};

fn make_user_stats_received(game_id: u64, result: i32, user_steam_id: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(20);
    payload.extend_from_slice(&game_id.to_le_bytes());
    payload.extend_from_slice(&result.to_le_bytes());
    payload.extend_from_slice(&user_steam_id.to_le_bytes());
    payload
}

fn make_user_stats_stored(game_id: u64, result: i32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&game_id.to_le_bytes());
    payload.extend_from_slice(&result.to_le_bytes());
    payload
}

#[test]
fn steam_result_ok_round_trip() {
    let r = SteamResult::Ok;
    assert_eq!(r.raw(), 1);
    assert!(r.is_ok());
    assert_eq!(SteamResult::from_raw(1), SteamResult::Ok);
    assert!(SteamResult::from_raw(1).is_ok());
}

#[test]
fn steam_result_other_preserves_code() {
    let r = SteamResult::from_raw(42);
    assert_eq!(r.raw(), 42);
    assert!(!r.is_ok());
    assert_eq!(r, SteamResult::Other(42));
}

#[test]
fn steam_result_zero_is_other_not_ok() {
    let r = SteamResult::from_raw(0);
    assert!(!r.is_ok());
    assert_eq!(r.raw(), 0);
}

#[test]
fn decode_user_stats_received_ok() {
    let payload = make_user_stats_received(480, 1, 76561198000000001);
    let raw = RawCallback { id: 1101, payload };
    let cb = steamlens_core::SteamCallback::from(raw);
    match cb {
        SteamCallback::UserStatsReceived {
            game_id,
            result,
            user_steam_id,
        } => {
            assert_eq!(game_id, 480);
            assert!(result.is_ok());
            assert_eq!(user_steam_id, 76561198000000001);
        }
        other => panic!("expected UserStatsReceived, got {other:?}"),
    }
}

#[test]
fn decode_user_stats_received_failure_result() {
    let payload = make_user_stats_received(480, 15, 76561198000000001);
    let raw = RawCallback { id: 1101, payload };
    let cb = SteamCallback::from(raw);
    match cb {
        SteamCallback::UserStatsReceived { result, .. } => {
            assert!(!result.is_ok());
            assert_eq!(result.raw(), 15);
        }
        other => panic!("expected UserStatsReceived, got {other:?}"),
    }
}

#[test]
fn decode_user_stats_received_truncated_returns_unknown() {
    let payload = make_user_stats_received(480, 1, 76561198000000001);
    let truncated = payload[..19].to_vec();
    let raw = RawCallback {
        id: 1101,
        payload: truncated,
    };
    let cb = SteamCallback::from(raw);
    assert!(
        matches!(cb, SteamCallback::Unknown(_)),
        "truncated UserStatsReceived must fall back to Unknown, got {cb:?}"
    );
}

#[test]
fn decode_user_stats_stored_ok() {
    let payload = make_user_stats_stored(480, 1);
    let raw = RawCallback { id: 1102, payload };
    let cb = SteamCallback::from(raw);
    match cb {
        SteamCallback::UserStatsStored { game_id, result } => {
            assert_eq!(game_id, 480);
            assert!(result.is_ok());
        }
        other => panic!("expected UserStatsStored, got {other:?}"),
    }
}

#[test]
fn decode_user_stats_stored_truncated_returns_unknown() {
    let payload = make_user_stats_stored(480, 1);
    let truncated = payload[..11].to_vec();
    let raw = RawCallback {
        id: 1102,
        payload: truncated,
    };
    let cb = SteamCallback::from(raw);
    assert!(
        matches!(cb, SteamCallback::Unknown(_)),
        "truncated UserStatsStored must fall back to Unknown, got {cb:?}"
    );
}

#[test]
fn unknown_callback_id_returns_unknown_variant() {
    let raw = RawCallback {
        id: 9999,
        payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let cb = SteamCallback::from(raw.clone());
    match cb {
        SteamCallback::Unknown(inner) => {
            assert_eq!(inner.id, 9999);
            assert_eq!(inner.payload, [0xDE, 0xAD, 0xBE, 0xEF]);
        }
        other => panic!("expected Unknown for id=9999, got {other:?}"),
    }
}

#[test]
fn unknown_callback_empty_payload_round_trips() {
    let raw = RawCallback {
        id: 42,
        payload: vec![],
    };
    let cb = SteamCallback::from(raw);
    assert!(matches!(cb, SteamCallback::Unknown(_)));
}

#[test]
fn decode_user_stats_received_exact_min_len() {
    let game_id: u64 = 105600;
    let result: i32 = 1;
    let user_steam_id: u64 = 76561198000000042;

    let mut payload = Vec::with_capacity(20);
    payload.extend_from_slice(&game_id.to_le_bytes());
    payload.extend_from_slice(&result.to_le_bytes());
    payload.extend_from_slice(&user_steam_id.to_le_bytes());
    assert_eq!(payload.len(), 20);

    let raw = RawCallback { id: 1101, payload };
    let cb = SteamCallback::from(raw);
    match cb {
        SteamCallback::UserStatsReceived {
            game_id: decoded_game_id,
            result: decoded_result,
            user_steam_id: decoded_user_steam_id,
        } => {
            assert_eq!(decoded_game_id, game_id);
            assert!(decoded_result.is_ok());
            assert_eq!(decoded_user_steam_id, user_steam_id);
        }
        other => panic!("expected UserStatsReceived at exact min size, got {other:?}"),
    }
}

#[test]
fn decode_user_stats_stored_exact_min_len() {
    let game_id: u64 = 105600;
    let result: i32 = 1;

    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&game_id.to_le_bytes());
    payload.extend_from_slice(&result.to_le_bytes());
    assert_eq!(payload.len(), 12);

    let raw = RawCallback { id: 1102, payload };
    let cb = SteamCallback::from(raw);
    match cb {
        SteamCallback::UserStatsStored {
            game_id: decoded_game_id,
            result: decoded_result,
        } => {
            assert_eq!(decoded_game_id, game_id);
            assert!(decoded_result.is_ok());
        }
        other => panic!("expected UserStatsStored at exact min size, got {other:?}"),
    }
}
