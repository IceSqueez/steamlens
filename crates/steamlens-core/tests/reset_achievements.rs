// Integration tests for achievement listing and destructive reset.
//
// These tests require a live Steam client signed in on the host machine.
// All tests are #[ignore]d — you must opt in explicitly.
//
// ── list_all (read-only, safe) ────────────────────────────────────────────────
//
//   SteamAppId=105600 cargo test -p steamlens-core --test reset_achievements \
//       list_all -- --ignored --nocapture
//
//   Prints every achievement name + display name + unlocked status.
//   Does not modify any data. Safe to run on any game.
//   Suggested apps: 105600 (Terraria), 480 (Spacewar — free test app).
//
// ── __destructive_clear_all_DO_NOT_RUN_ON_FAVORITE_GAME (destructive) ─────────
//
//   STEAMLENS_DESTRUCTIVE_OK=yes SteamAppId=105600 \
//       cargo test -p steamlens-core --test reset_achievements \
//       __destructive_clear_all_DO_NOT_RUN_ON_FAVORITE_GAME \
//       -- --ignored --nocapture
//
//   WARNING: WIPES ALL ACHIEVEMENTS AND STATS PERMANENTLY for the app
//   specified by SteamAppId. Steam does not provide an undo mechanism.
//   Only run this against a game whose achievement progress you are willing
//   to lose. Suggested safe choices: Spacewar (480) — it is a Valve test app.
//
// Three levels of opt-in for the destructive test:
//   1. Long, intentionally inconvenient test name.
//   2. STEAMLENS_DESTRUCTIVE_OK=yes env var must be set.
//   3. SteamAppId must be set to an explicit app id.

use std::time::Duration;

use steamlens_core::{Client, SteamCallback, SteamResult, connect};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn wait_for_user_stats_received(
    client: &Client,
    expected_user: u64,
    timeout: Duration,
) -> Result<(), String> {
    let poll_interval = Duration::from_millis(100);
    let iterations = (timeout.as_millis() / poll_interval.as_millis()).max(1) as u64;

    for i in 0..iterations {
        let callbacks = client
            .poll_callbacks()
            .map_err(|e| format!("poll_callbacks failed on iteration {i}: {e}"))?;

        for cb in &callbacks {
            if let SteamCallback::UserStatsReceived {
                result,
                user_steam_id,
                ..
            } = cb
                && *user_steam_id == expected_user
            {
                return if result.is_ok() {
                    Ok(())
                } else {
                    Err(format!(
                        "UserStatsReceived arrived but result was not Ok: {result:?}"
                    ))
                };
            }
        }

        std::thread::sleep(poll_interval);
    }

    Err(format!(
        "UserStatsReceived not received within {}ms",
        timeout.as_millis()
    ))
}

fn wait_for_user_stats_stored(client: &Client, timeout: Duration) -> Result<SteamResult, String> {
    let poll_interval = Duration::from_millis(100);
    let iterations = (timeout.as_millis() / poll_interval.as_millis()).max(1) as u64;

    for i in 0..iterations {
        let callbacks = client
            .poll_callbacks()
            .map_err(|e| format!("poll_callbacks failed on iteration {i}: {e}"))?;

        for cb in &callbacks {
            if let SteamCallback::UserStatsStored { result, .. } = cb {
                return Ok(*result);
            }
        }

        std::thread::sleep(poll_interval);
    }

    Err(format!(
        "UserStatsStored not received within {}ms",
        timeout.as_millis()
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires Steam running; read-only achievement listing for the app set by SteamAppId"]
fn list_all() {
    let client = connect().expect("connect() must succeed with Steam running");
    let stats = client.user_stats();
    let steam_id = client.steam_id();

    println!("steam_id = {steam_id}");

    stats
        .request_user_stats(steam_id)
        .expect("request_user_stats must not fail when Steam is running");

    println!("RequestUserStats dispatched — waiting for UserStatsReceived...");

    wait_for_user_stats_received(&client, steam_id, Duration::from_secs(5))
        .expect("UserStatsReceived must arrive within 5 seconds");

    let count = stats
        .num_achievements()
        .expect("num_achievements must not fail after UserStatsReceived");

    println!("Total achievements: {count}");

    for i in 0..count {
        let name = stats
            .achievement_name(i)
            .unwrap_or_else(|_| format!("<index {i} out of range>"));

        let achieved = stats.get_achievement(&name).unwrap_or(false);

        let display = stats
            .achievement_display_attribute(&name, "name")
            .unwrap_or_else(|_| "(no display name)".into());

        let status = if achieved { "\u{2713}" } else { "\u{00b7}" };
        println!("  [{status}] {name} \u{2014} {display}");
    }
}

#[test]
#[allow(non_snake_case)]
#[ignore = "DESTRUCTIVE: WIPES ALL ACHIEVEMENTS AND STATS for SteamAppId permanently. \
            Requires STEAMLENS_DESTRUCTIVE_OK=yes AND SteamAppId env vars. \
            Do not run on a game you care about."]
fn __destructive_clear_all_DO_NOT_RUN_ON_FAVORITE_GAME() {
    if std::env::var("STEAMLENS_DESTRUCTIVE_OK").as_deref() != Ok("yes") {
        panic!(
            "Refusing to run destructive test. Set STEAMLENS_DESTRUCTIVE_OK=yes \
             AND SteamAppId=<your_test_app> if you really want this. \
             Picking a beloved game's appid here will wipe its achievements and stats."
        );
    }

    let app_id = std::env::var("SteamAppId")
        .expect("SteamAppId env var must be set for the destructive test");

    println!("Destructive test: resetting all achievements and stats for app {app_id}");

    let client = connect().expect("connect() must succeed with Steam running");
    let stats = client.user_stats();
    let steam_id = client.steam_id();

    println!("steam_id = {steam_id}");

    stats
        .request_user_stats(steam_id)
        .expect("request_user_stats must not fail when Steam is running");

    println!("RequestUserStats dispatched — waiting for UserStatsReceived...");

    wait_for_user_stats_received(&client, steam_id, Duration::from_secs(5))
        .expect("UserStatsReceived must arrive within 5 seconds");

    let count = stats
        .num_achievements()
        .expect("num_achievements must not fail after UserStatsReceived");

    let first_name = (count > 0)
        .then(|| stats.achievement_name(0).ok())
        .flatten();
    let before = first_name
        .as_deref()
        .and_then(|n| stats.get_achievement(n).ok());

    println!(
        "Before reset: {count} achievements defined, \
         first={first_name:?} achieved={before:?}"
    );

    stats
        .reset_all_stats(true)
        .expect("reset_all_stats must not fail after UserStatsReceived");

    println!("ResetAllStats(true) staged — calling store_stats()...");

    stats
        .store_stats()
        .expect("store_stats must not fail after reset_all_stats");

    println!("StoreStats dispatched — waiting for UserStatsStored...");

    let stored = wait_for_user_stats_stored(&client, Duration::from_secs(5))
        .expect("UserStatsStored must arrive within 5 seconds");

    println!("UserStatsStored: result={stored:?}");

    assert!(
        stored.is_ok(),
        "UserStatsStored result must be Ok, got {stored:?}"
    );

    let after = first_name
        .as_deref()
        .and_then(|n| stats.get_achievement(n).ok());

    println!(
        "After reset: first={first_name:?} achieved={after:?} \
         (expect false; stat-driven achievements will stay locked on next game run)"
    );

    if let Some(false) = after {
        println!("OK: first achievement is now locked.");
    } else if after.is_none() {
        println!("(no achievements defined for this app — reset dispatched successfully)");
    }
}
