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

// ── Schema helpers ────────────────────────────────────────────────────────────

#[derive(Debug)]
struct ProgressEntry {
    achievement_name: String,
    max_val: u32,
}

fn schema_path(app_id: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").expect("HOME env var must be set");
    std::path::PathBuf::from(home)
        .join(".local/share/Steam/appcache/stats")
        .join(format!("UserGameStatsSchema_{app_id}.bin"))
}

fn parse_progress_entries(schema: &steamlens_vdf::Value, app_id: &str) -> Vec<ProgressEntry> {
    let stats_section = match schema.get(&format!("{app_id}/stats")) {
        Some(v) => v,
        None => {
            eprintln!("WARNING: no '{app_id}/stats' section in schema — skipping progress reset");
            return Vec::new();
        }
    };

    let stat_pairs = match stats_section.as_section() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    for stat_pair in stat_pairs {
        let stat_children = match stat_pair.value.as_section() {
            Some(c) => c,
            None => continue,
        };

        let type_str = stat_children
            .iter()
            .find(|p| p.key == "type")
            .and_then(|p| p.value.as_str())
            .unwrap_or("");

        if type_str != "ACHIEVEMENTS" {
            continue;
        }

        let bits_section = match stat_children.iter().find(|p| p.key == "bits") {
            Some(b) => b,
            None => continue,
        };

        let bits = match bits_section.value.as_section() {
            Some(b) => b,
            None => continue,
        };

        for bit in bits {
            let bit_children = match bit.value.as_section() {
                Some(c) => c,
                None => continue,
            };

            let name = bit_children
                .iter()
                .find(|p| p.key == "name")
                .and_then(|p| p.value.as_str())
                .unwrap_or("")
                .to_owned();

            if name.is_empty() {
                continue;
            }

            let progress_section = match bit_children.iter().find(|p| p.key == "progress") {
                Some(p) => p,
                None => continue,
            };

            let progress_children = match progress_section.value.as_section() {
                Some(c) => c,
                None => continue,
            };

            let max_val = progress_children
                .iter()
                .find(|p| p.key == "max_val")
                .and_then(|p| p.value.as_i32())
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);

            if max_val == 0 {
                continue;
            }

            entries.push(ProgressEntry {
                achievement_name: name,
                max_val,
            });
        }
    }

    entries
}

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
    let client = connect(0).expect("connect(0) must succeed with Steam running");
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

    let app_id_u32: u32 = app_id
        .parse()
        .expect("SteamAppId must be a valid u32 (e.g. 105600)");

    println!("Destructive test: resetting all achievements and stats for app {app_id}");

    // ── Phase 1: Load schema from disk ────────────────────────────────────────

    let schema_file = schema_path(&app_id);
    let progress_entries = match std::fs::read(&schema_file) {
        Ok(bytes) => match steamlens_vdf::parse(&bytes) {
            Ok(schema) => {
                let entries = parse_progress_entries(&schema, &app_id);
                println!("Schema loaded from {}", schema_file.display());
                println!(
                    "Found {} achievement(s) with stat-driven progress counters:",
                    entries.len()
                );
                for e in &entries {
                    println!("  {} (max={})", e.achievement_name, e.max_val);
                }
                entries
            }
            Err(e) => {
                println!("WARNING: schema parse failed ({e}) — skipping progress counter reset");
                Vec::new()
            }
        },
        Err(e) => {
            println!(
                "WARNING: schema file not found at {} ({e}) — \
                 skipping progress counter reset",
                schema_file.display()
            );
            Vec::new()
        }
    };

    // ── Phase 2: Connect + initial stats load ─────────────────────────────────

    let client = connect(app_id_u32).expect("connect(app_id) must succeed with Steam running");
    let stats = client.user_stats();
    let steam_id = client.steam_id();

    println!("steam_id = {steam_id}");

    println!(
        "\n>>> CHECKPOINT 1: Sleeping 2s after connect({app_id_u32}). \
         Look at Steam UI — does it show '{app_id} — In Game' or your friends list status \
         changed to 'In-Game Terraria' (or whatever app you used)?"
    );
    std::thread::sleep(Duration::from_secs(2));

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

    // ── Phase 3: ResetAllStats + store ────────────────────────────────────────

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

    // ── Phase 4: Reset Steam's internal popup counters via IndicateAchievementProgress ──

    if progress_entries.is_empty() {
        println!("\nNo schema-derived progress entries — skipping popup counter reset.");
    } else {
        println!(
            "\nResetting {} stat-driven achievement popup counter(s) via \
             IndicateAchievementProgress(name, 0, max)...",
            progress_entries.len()
        );

        let mut reset_ok = 0usize;
        let mut reset_fail = 0usize;

        for entry in &progress_entries {
            match stats.indicate_achievement_progress(&entry.achievement_name, 0, entry.max_val) {
                Ok(()) => {
                    println!("  [OK]   {} (0/{})", entry.achievement_name, entry.max_val);
                    reset_ok += 1;
                }
                Err(e) => {
                    println!(
                        "  [FAIL] {} (0/{}) — {e}",
                        entry.achievement_name, entry.max_val
                    );
                    reset_fail += 1;
                }
            }
        }

        // ── Phase 5: store_stats again to persist popup counter state ─────────

        println!("\nCalling store_stats() to persist popup counter resets...");

        stats
            .store_stats()
            .expect("second store_stats must not fail");

        println!("Waiting for second UserStatsStored...");

        let stored2 = wait_for_user_stats_stored(&client, Duration::from_secs(5))
            .expect("second UserStatsStored must arrive within 5 seconds");

        println!("Second UserStatsStored: result={stored2:?}");

        // ── Phase 6: summary ──────────────────────────────────────────────────

        println!(
            "\nSummary: Reset {reset_ok} stat-driven achievement popup counter(s) OK, \
             {reset_fail} failed."
        );

        if reset_fail > 0 {
            println!(
                "NOTE: Failures typically mean Steam rejected the name (schema mismatch) or \
                 max_val was zero. This does not affect the regular stat/achievement reset \
                 which completed successfully in Phase 3."
            );
        }
    }

    println!(
        "\n>>> CHECKPOINT 2: Sleeping 2s before disconnect. \
         Look at Steam UI again — friends-list status, popups, anything change?"
    );
    std::thread::sleep(Duration::from_secs(2));
}
