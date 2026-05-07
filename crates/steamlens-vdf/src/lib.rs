mod localconfig;
mod packageinfo;
mod parser;
pub mod text;

pub use localconfig::parse_localconfig_last_played;
pub use packageinfo::{PackageInfoError, parse_packageinfo};
pub use parser::{KeyValuePair, Value, VdfError};
pub use text::{TextValue, TextVdfError, parse as parse_text};

/// Parse a binary KV blob (`appcache/stats/UserGameStatsSchema_*.bin`,
/// `packageinfo.vdf` records) into a root [`Value::Section`].
pub fn parse(bytes: &[u8]) -> Result<Value, VdfError> {
    let mut cursor = parser::Cursor::new(bytes);
    cursor.read_section()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section_bytes(inner: &[u8]) -> Vec<u8> {
        let mut v = inner.to_vec();
        v.push(0x08);
        v
    }

    fn string_entry(key: &str, val: &str) -> Vec<u8> {
        let mut v = vec![0x01];
        v.extend_from_slice(key.as_bytes());
        v.push(0x00);
        v.extend_from_slice(val.as_bytes());
        v.push(0x00);
        v
    }

    fn i32_entry(key: &str, val: i32) -> Vec<u8> {
        let mut v = vec![0x02];
        v.extend_from_slice(key.as_bytes());
        v.push(0x00);
        v.extend_from_slice(&val.to_le_bytes());
        v
    }

    fn u64_entry(key: &str, val: u64) -> Vec<u8> {
        let mut v = vec![0x07];
        v.extend_from_slice(key.as_bytes());
        v.push(0x00);
        v.extend_from_slice(&val.to_le_bytes());
        v
    }

    fn f32_entry(key: &str, val: f32) -> Vec<u8> {
        let mut v = vec![0x03];
        v.extend_from_slice(key.as_bytes());
        v.push(0x00);
        v.extend_from_slice(&val.to_le_bytes());
        v
    }

    fn section_entry(key: &str, inner: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00];
        v.extend_from_slice(key.as_bytes());
        v.push(0x00);
        v.extend_from_slice(inner);
        v.push(0x08);
        v
    }

    #[test]
    fn parse_empty_input_errors() {
        let err = parse(&[]).unwrap_err();
        assert!(matches!(err, VdfError::UnexpectedEof { .. }));
    }

    #[test]
    fn parse_only_end_marker() {
        let result = parse(&[0x08]).unwrap();
        assert_eq!(result, Value::Section(vec![]));
    }

    #[test]
    fn parse_simple_string() {
        let mut bytes = string_entry("key", "val");
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(
            result,
            Value::Section(vec![KeyValuePair {
                key: "key".into(),
                value: Value::String("val".into()),
            }])
        );
    }

    #[test]
    fn parse_int32() {
        let mut bytes = i32_entry("n", -42_i32);
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(
            result,
            Value::Section(vec![KeyValuePair {
                key: "n".into(),
                value: Value::Int32(-42),
            }])
        );
    }

    #[test]
    fn parse_uint64() {
        let val: u64 = 0xDEAD_BEEF_CAFE_1234;
        let mut bytes = u64_entry("id", val);
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(
            result,
            Value::Section(vec![KeyValuePair {
                key: "id".into(),
                value: Value::UInt64(val),
            }])
        );
    }

    #[test]
    fn parse_float32() {
        let val: f32 = 1.5_f32;
        let mut bytes = f32_entry("f", val);
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        match &result {
            Value::Section(pairs) => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0].key, "f");
                assert_eq!(pairs[0].value, Value::Float32(val));
            }
            _ => panic!("expected Section"),
        }
    }

    #[test]
    fn parse_nested_sections() {
        let leaf = section_bytes(&string_entry("a", "b"));
        let inner = section_entry("inner", &leaf);
        let mut outer = section_entry("outer", &inner);
        outer.push(0x08);

        let result = parse(&outer).unwrap();
        let Value::Section(top) = &result else {
            panic!("expected top-level Section");
        };
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].key, "outer");

        let Value::Section(mid) = &top[0].value else {
            panic!("expected mid-level Section");
        };
        assert_eq!(mid.len(), 1);
        assert_eq!(mid[0].key, "inner");

        let Value::Section(leaf_pairs) = &mid[0].value else {
            panic!("expected leaf Section");
        };
        assert_eq!(leaf_pairs.len(), 1);
        assert_eq!(leaf_pairs[0].key, "a");
        assert_eq!(leaf_pairs[0].value, Value::String("b".into()));
    }

    #[test]
    fn parse_unknown_tag_errors() {
        let bytes = [0x99, b'k', 0x00, 0x08];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, VdfError::UnknownTypeTag { tag: 0x99, .. }));
    }

    #[test]
    fn parse_wstring_unsupported() {
        let bytes = [0x05, b'k', 0x00, 0x08];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, VdfError::UnsupportedType { tag: 0x05, .. }));
    }

    #[test]
    fn parse_truncated_string_errors() {
        let bytes = [0x01, b'k', 0x00, b'v', b'a', b'l'];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, VdfError::UnexpectedEof { .. }));
    }

    #[test]
    fn parse_truncated_int32_errors() {
        let bytes = [0x02, b'n', 0x00, 0x01, 0x02, 0x03];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, VdfError::UnexpectedEof { .. }));
    }

    #[test]
    fn parse_invalid_utf8_in_key_errors() {
        let bytes = [0x01, 0xFF, 0xFE, 0x00, b'v', 0x00, 0x08];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, VdfError::InvalidUtf8 { .. }));
    }

    #[test]
    fn parse_empty_string_value() {
        let mut bytes = vec![0x01];
        bytes.extend_from_slice(b"k\x00");
        bytes.push(0x00);
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(
            result,
            Value::Section(vec![KeyValuePair {
                key: "k".into(),
                value: Value::String(String::new()),
            }])
        );
    }

    #[test]
    fn value_get_path() {
        let inner = Value::Section(vec![KeyValuePair {
            key: "leaf".into(),
            value: Value::String("hello".into()),
        }]);
        let root = Value::Section(vec![KeyValuePair {
            key: "mid".into(),
            value: inner,
        }]);
        assert_eq!(root.get("mid/leaf"), Some(&Value::String("hello".into())));
        assert_eq!(
            root.get("mid"),
            Some(&Value::Section(vec![KeyValuePair {
                key: "leaf".into(),
                value: Value::String("hello".into()),
            }]))
        );
        assert_eq!(root.get("missing"), None);
        assert_eq!(root.get("mid/missing"), None);
    }

    #[test]
    fn value_as_helpers() {
        let s = Value::String("text".into());
        assert_eq!(s.as_str(), Some("text"));
        assert_eq!(s.as_i32(), None);

        let i = Value::Int32(7);
        assert_eq!(i.as_i32(), Some(7));
        assert_eq!(i.as_str(), None);

        let u = Value::UInt64(99);
        assert_eq!(u.as_u64(), Some(99));

        let f = Value::Float32(1.5);
        assert_eq!(f.as_f32(), Some(1.5));

        let sec = Value::Section(vec![]);
        assert_eq!(sec.as_section(), Some([].as_slice()));
    }

    #[test]
    fn read_array_i32_round_trip() {
        let val: i32 = -12345;
        let mut bytes = vec![0x02u8];
        bytes.extend_from_slice(b"n\x00");
        bytes.extend_from_slice(&val.to_le_bytes());
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(result.get("n").and_then(|v| v.as_i32()), Some(val));
    }

    #[test]
    fn read_array_u32_as_u64_round_trip() {
        let val: u32 = 0xDEAD_BEEF;
        let mut bytes = vec![0x04u8];
        bytes.extend_from_slice(b"u\x00");
        bytes.extend_from_slice(&val.to_le_bytes());
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(
            result.get("u").and_then(|v| v.as_u64()),
            Some(u64::from(val))
        );
    }

    #[test]
    fn read_array_f32_round_trip() {
        let val: f32 = std::f32::consts::FRAC_1_PI;
        let mut bytes = vec![0x03u8];
        bytes.extend_from_slice(b"f\x00");
        bytes.extend_from_slice(&val.to_le_bytes());
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(result.get("f").and_then(|v| v.as_f32()), Some(val));
    }

    #[test]
    fn read_array_u64_round_trip() {
        let val: u64 = 0xCAFE_BABE_DEAD_BEEF;
        let mut bytes = vec![0x07u8];
        bytes.extend_from_slice(b"x\x00");
        bytes.extend_from_slice(&val.to_le_bytes());
        bytes.push(0x08);
        let result = parse(&bytes).unwrap();
        assert_eq!(result.get("x").and_then(|v| v.as_u64()), Some(val));
    }
}
