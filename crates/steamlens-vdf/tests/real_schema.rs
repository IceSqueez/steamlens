use steamlens_vdf::Value;

// Hand-built bytes shaped like UserGameStatsSchema_<appid>.bin:
// 325180 -> { stats -> { "1" -> { bits -> {}, type = "ACHIEVEMENTS" } }, version = i32(6) }
#[rustfmt::skip]
static SCHEMA: &[u8] = &[
    0x00, b'3', b'2', b'5', b'1', b'8', b'0', 0x00,
        0x00, b's', b't', b'a', b't', b's', 0x00,
            0x00, b'1', 0x00,
                0x00, b'b', b'i', b't', b's', 0x00,
                0x08,
                0x01, b't', b'y', b'p', b'e', 0x00,
                    b'A', b'C', b'H', b'I', b'E', b'V', b'E',
                    b'M', b'E', b'N', b'T', b'S', 0x00,
            0x08,
        0x08,
        0x02, b'v', b'e', b'r', b's', b'i', b'o', b'n', 0x00,
            0x06, 0x00, 0x00, 0x00,
    0x08,
    0x08,
];

#[test]
fn parses_schema_shape_without_panic() {
    let root = steamlens_vdf::parse(SCHEMA).expect("schema must parse without error");

    let Value::Section(top) = &root else {
        panic!("expected top-level Section, got {root:?}");
    };

    assert!(!top.is_empty(), "root section must have at least one child");

    let appid_entry = &top[0];
    assert_eq!(appid_entry.key, "325180");

    let Value::Section(app_children) = &appid_entry.value else {
        panic!("expected app-id child to be a Section");
    };

    let has_stats = app_children.iter().any(|p| p.key == "stats");
    assert!(has_stats, "app section must contain a 'stats' child");
}

#[test]
fn path_walk_on_schema_shape() {
    let root = steamlens_vdf::parse(SCHEMA).unwrap();

    let achievement_type = root.get("325180/stats/1/type");
    assert_eq!(
        achievement_type,
        Some(&Value::String("ACHIEVEMENTS".into()))
    );

    let bits = root.get("325180/stats/1/bits");
    assert_eq!(bits, Some(&Value::Section(vec![])));

    let version = root.get("325180/version");
    assert_eq!(version, Some(&Value::Int32(6)));
}

#[test]
fn truncated_schema_returns_error() {
    let truncated = &SCHEMA[..20];
    let result = steamlens_vdf::parse(truncated);
    assert!(result.is_err());
}
