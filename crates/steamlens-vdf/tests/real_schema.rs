use steamlens_vdf::Value;

// Synthetic bytes that mimic the structure of a Steam UserGameStatsSchema_*.bin
// file: an outer section keyed by an app id, containing a nested "stats" tree
// (with an empty "bits" sub-section and a "type" leaf string), plus a sibling
// "version" Int32 leaf. This is the same shape as a real Steam schema, hand
// written so it lives in source as reviewable bytes (no real Steam data on
// disk, no PII, no opaque fixture file).
//
// Structure:
//   "325180" -> Section
//     "stats" -> Section
//       "1" -> Section
//         "bits" -> Section (empty)
//         "type" -> String "ACHIEVEMENTS"
//     "version" -> Int32(6)
#[rustfmt::skip]
static SCHEMA: &[u8] = &[
    0x00, b'3', b'2', b'5', b'1', b'8', b'0', 0x00,                // Section "325180"
        0x00, b's', b't', b'a', b't', b's', 0x00,                  //   Section "stats"
            0x00, b'1', 0x00,                                      //     Section "1"
                0x00, b'b', b'i', b't', b's', 0x00,                //       Section "bits"
                0x08,                                              //       End of "bits"
                0x01, b't', b'y', b'p', b'e', 0x00,                //       String "type"
                    b'A', b'C', b'H', b'I', b'E', b'V', b'E',
                    b'M', b'E', b'N', b'T', b'S', 0x00,            //         "ACHIEVEMENTS"
            0x08,                                                  //     End of "1"
        0x08,                                                      //   End of "stats"
        0x02, b'v', b'e', b'r', b's', b'i', b'o', b'n', 0x00,      //   Int32 "version"
            0x06, 0x00, 0x00, 0x00,                                //     6 (LE)
    0x08,                                                          // End of "325180"
    0x08,                                                          // End of top
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

    // Structure: 325180 -> stats -> 1 -> type = "ACHIEVEMENTS"
    let achievement_type = root.get("325180/stats/1/type");
    assert_eq!(
        achievement_type,
        Some(&Value::String("ACHIEVEMENTS".into()))
    );

    // The "bits" sub-section is empty — exercises the empty-section parse path.
    let bits = root.get("325180/stats/1/bits");
    assert_eq!(bits, Some(&Value::Section(vec![])));

    // version is a sibling of "stats" under the "325180" app section
    let version = root.get("325180/version");
    assert_eq!(version, Some(&Value::Int32(6)));
}

#[test]
fn truncated_schema_returns_error() {
    // A sub-slice that ends abruptly mid-record.
    let truncated = &SCHEMA[..20];
    let result = steamlens_vdf::parse(truncated);
    assert!(
        result.is_err(),
        "truncated input must produce an error, not Ok"
    );
}
