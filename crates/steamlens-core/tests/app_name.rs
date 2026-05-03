/// Integration test: verify that `Client::app_name()` returns a recognisable
/// display name for Terraria (app ID 105600).
///
/// Requires a running Steam client with Terraria in the library.
///
/// Run with:
///
/// ```text
/// cargo test -p steamlens-core --test app_name -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn app_name_terraria() {
    let client = steamlens_core::connect(105600)
        .expect("Steam must be running with Terraria (105600) in the library");

    let name = client.app_name();
    println!("app_name() = {:?}", name);

    let name = name.expect("app_name() must return Some for a known app");
    assert!(
        name.to_lowercase().contains("terra"),
        "expected name containing 'terra', got: {name:?}"
    );
}

/// Smoke test: `app_id == 0` must return `None` without touching Steam.
///
/// Run with:
///
/// ```text
/// cargo test -p steamlens-core --test app_name -- app_name_zero_app_id --ignored --nocapture
/// ```
#[test]
#[ignore]
fn app_name_zero_app_id() {
    let client = steamlens_core::connect(0).expect("Steam must be running for this test");

    let name = client.app_name();
    println!("app_name() with app_id=0: {:?}", name);
    assert!(
        name.is_none(),
        "app_name() must return None when app_id is 0"
    );
}
