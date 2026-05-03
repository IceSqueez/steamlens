/// A single Steam callback message with its payload copied into owned memory.
///
/// The `id` field identifies the callback type (e.g. 1701 for
/// `SteamServersConnected`). `payload` contains the raw bytes of the callback
/// parameter struct as written by Steam; callers that know the concrete type
/// for a given `id` may reinterpret the slice with `unsafe { &*(payload.as_ptr()
/// as *const ConcreteStruct) }`.
///
/// The payload is owned — Steam's internal buffer has already been freed by
/// the time this value is visible to callers. There is no use-after-free risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCallback {
    pub id: i32,
    pub payload: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::RawCallback;

    #[test]
    fn empty_callback_round_trips_through_clone_and_eq() {
        let cb = RawCallback {
            id: 0,
            payload: vec![],
        };
        let cloned = cb.clone();
        assert_eq!(cb, cloned);
        assert_eq!(cb.id, 0);
        assert!(cb.payload.is_empty());
    }

    #[test]
    fn callback_with_payload_round_trips() {
        let cb = RawCallback {
            id: 1701,
            payload: vec![0x01, 0x02, 0x03, 0x04],
        };
        let cloned = cb.clone();
        assert_eq!(cb, cloned);
        assert_eq!(cloned.id, 1701);
        assert_eq!(cloned.payload, [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn different_ids_are_not_equal() {
        let a = RawCallback {
            id: 1,
            payload: vec![],
        };
        let b = RawCallback {
            id: 2,
            payload: vec![],
        };
        assert_ne!(a, b);
    }

    #[test]
    fn different_payloads_are_not_equal() {
        let a = RawCallback {
            id: 42,
            payload: vec![0],
        };
        let b = RawCallback {
            id: 42,
            payload: vec![1],
        };
        assert_ne!(a, b);
    }

    #[test]
    fn debug_output_contains_id() {
        let cb = RawCallback {
            id: 999,
            payload: vec![],
        };
        let s = format!("{cb:?}");
        assert!(s.contains("999"), "Debug output missing id: {s}");
    }
}
