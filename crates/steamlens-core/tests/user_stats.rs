// Integration test for the ISteamUserStats013 sync wrapper.
//
// Requires a live Steam client. Run with:
//   cargo test -p steamlens-core --test user_stats -- --ignored --nocapture
//
// All tests in this file are `#[ignore]`d so they do not run in CI.
//
// IMPORTANT: This test is read-only. It does NOT call set_achievement,
// clear_achievement, or store_stats to avoid corrupting the real user's game data.
//
// Note: Without a prior RequestUserStats call (Round 2), get_* methods may
// return Steam defaults (0 / false). The test prints values for inspection
// but does not assert on specific numbers — schema availability depends on
// which app is active in the current Steam session.

use steamlens_core::connect;

/// Verifies that user_stats() is reachable from a live Client and that the
/// basic schema-browsing methods (num_achievements, achievement_name,
/// get_achievement, achievement_display_attribute) do not panic or error on a
/// connected session.
#[test]
#[ignore]
fn user_stats_schema_browse() {
    let client = connect().expect("Steam must be running");
    let stats = client.user_stats();

    let count = stats.num_achievements().expect("num_achievements failed");
    println!("num_achievements = {count}");

    if count == 0 {
        println!("No achievements in schema for this session — skipping per-achievement checks.");
        println!("Hint: set SteamAppId env var to a game with achievements before running.");
        return;
    }

    let name = stats
        .achievement_name(0)
        .expect("achievement_name(0) failed");
    println!("achievement_name(0) = {name:?}");

    match stats.get_achievement(&name) {
        Ok(achieved) => println!("get_achievement({name:?}) = {achieved}"),
        Err(e) => println!(
            "get_achievement({name:?}) returned Err (expected before RequestUserStats): {e}"
        ),
    }

    match stats.achievement_display_attribute(&name, "name") {
        Ok(display_name) => println!("display name = {display_name:?}"),
        Err(e) => println!("display_attribute name: {e}"),
    }

    match stats.achievement_display_attribute(&name, "desc") {
        Ok(desc) => println!("display desc = {desc:?}"),
        Err(e) => println!("display_attribute desc: {e}"),
    }

    match stats.achievement_display_attribute(&name, "hidden") {
        Ok(hidden) => println!("hidden = {hidden:?}"),
        Err(e) => println!("display_attribute hidden: {e}"),
    }

    let icon_handle = stats
        .achievement_icon(&name)
        .expect("achievement_icon failed");
    println!("achievement_icon({name:?}) = {icon_handle} (0 = not yet loaded)");
}

/// Verifies that achievement_name returns AchievementNotFound for an
/// out-of-range index rather than panicking.
#[test]
#[ignore]
fn achievement_name_out_of_range() {
    let client = connect().expect("Steam must be running");
    let stats = client.user_stats();

    let result = stats.achievement_name(u32::MAX);
    println!("achievement_name(u32::MAX) = {result:?}");
    assert!(
        result.is_err(),
        "expected AchievementNotFound for u32::MAX index"
    );
}

/// Verifies that names containing interior NUL bytes produce InvalidString
/// rather than panicking or silently truncating.
#[test]
#[ignore]
fn invalid_string_returns_error() {
    let client = connect().expect("Steam must be running");
    let stats = client.user_stats();

    let result = stats.get_achievement("ACH\0IEVMENT");
    println!("get_achievement with NUL byte: {result:?}");
    assert!(
        matches!(
            result,
            Err(steamlens_core::SteamError::InvalidString { .. })
        ),
        "expected InvalidString"
    );
}

