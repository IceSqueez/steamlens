use steamlens_vdf::Value;

static FIXTURE: &[u8] = include_bytes!("fixtures/UserGameStatsSchema_325180.bin");

#[test]
fn parses_real_schema_without_panic() {
    let root = steamlens_vdf::parse(FIXTURE).expect("fixture must parse without error");

    // Top-level must be a Section.
    let Value::Section(top) = &root else {
        panic!("expected top-level Section, got {root:?}");
    };

    // The root section of a schema file contains one child whose key is
    // the numeric app id as a string ("325180").  That child is itself
    // a Section containing at least a "stats" sub-section.
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
fn path_walk_on_real_schema() {
    let root = steamlens_vdf::parse(FIXTURE).unwrap();

    // Structure: 325180 -> stats -> 1 -> type = "ACHIEVEMENTS"
    let achievement_type = root.get("325180/stats/1/type");
    assert_eq!(
        achievement_type,
        Some(&Value::String("ACHIEVEMENTS".into()))
    );

    // The "bits" sub-section is empty in this schema — exercises the
    // empty-section parse path explicitly.
    let bits = root.get("325180/stats/1/bits");
    assert_eq!(bits, Some(&Value::Section(vec![])));

    // version is a sibling of "stats" under the "325180" app section
    let version = root.get("325180/version");
    assert_eq!(version, Some(&Value::Int32(6)));
}

#[test]
fn truncated_fixture_returns_error() {
    // A sub-slice of the fixture that ends abruptly mid-record.
    let truncated = &FIXTURE[..20];
    let result = steamlens_vdf::parse(truncated);
    assert!(
        result.is_err(),
        "truncated input must produce an error, not Ok"
    );
}
