// Diagnostic integration test: verify that ResetAllStats(true) actually zeroes
// underlying integer and float stat counters, not just achievement flags.
//
// Background: after a reset+store cycle, Terraria's UI was still showing quest
// progress (21/25, 14/15, etc.) on a fresh character/world. This test reads
// every non-achievement stat value before and after the reset so we can
// determine whether:
//
//   (a) Our wrapper did not correctly call ResetAllStats — bug in steamlens-core.
//   (b) Steam reset the stats but the local cache was stale — transient.
//   (c) ResetAllStats genuinely does not zero these counters — Steam runtime
//       behaviour that we cannot control from our side.
//   (d) Terraria uses IndicateAchievementProgress (a separate internal Steam
//       progress counter) that ResetAllStats does not touch.
//
// Run with:
//
//   STEAMLENS_DESTRUCTIVE_OK=yes SteamAppId=105600 \
//       cargo test -p steamlens-core --test diagnose_reset \
//       diagnose_stat_reset_after_reset_all_stats \
//       -- --ignored --nocapture
//
// THREE opt-in levels:
//   1. Long, intentionally inconvenient test name.
//   2. STEAMLENS_DESTRUCTIVE_OK=yes — explicit env var.
//   3. SteamAppId must be set — prevents accidental wrong-game runs.
//
// WARNING: This test calls ResetAllStats(true) + StoreStats(). It permanently
// wipes all achievements and stats for the specified app. Do NOT use it with a
// game whose progress you care about.

use std::time::Duration;

use steamlens_core::{Client, SteamCallback, SteamResult, connect};
use steamlens_vdf::Value;

// ── Schema types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum StatKind {
    Int,
    Float,
}

#[derive(Debug, Clone)]
struct StatEntry {
    name: String,
    kind: StatKind,
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

fn schema_path(app_id: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").expect("HOME env var must be set");
    std::path::PathBuf::from(home)
        .join(".local/share/Steam/appcache/stats")
        .join(format!("UserGameStatsSchema_{app_id}.bin"))
}

fn parse_stat_entries(schema: &Value, app_id: &str) -> Vec<StatEntry> {
    let stats_section = match schema.get(&format!("{app_id}/stats")) {
        Some(v) => v,
        None => {
            eprintln!("WARNING: no '{app_id}/stats' section found in schema");
            return Vec::new();
        }
    };

    let pairs = match stats_section.as_section() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    for stat_pair in pairs {
        let stat_children = match stat_pair.value.as_section() {
            Some(c) => c,
            None => continue,
        };

        let type_str = stat_children
            .iter()
            .find(|p| p.key == "type")
            .and_then(|p| p.value.as_str())
            .unwrap_or("");

        let name = stat_children
            .iter()
            .find(|p| p.key == "name")
            .and_then(|p| p.value.as_str())
            .unwrap_or("")
            .to_owned();

        if name.is_empty() {
            continue;
        }

        let kind = match type_str {
            "INT" => StatKind::Int,
            "FLOAT" => StatKind::Float,
            _ => continue,
        };

        entries.push(StatEntry { name, kind });
    }

    entries
}

#[derive(Debug)]
enum StatValue {
    Int(i32),
    Float(f32),
    Error(String),
}

impl std::fmt::Display for StatValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatValue::Int(n) => write!(f, "{n}"),
            StatValue::Float(v) => write!(f, "{v:.6}"),
            StatValue::Error(e) => write!(f, "<error: {e}>"),
        }
    }
}

fn read_stat(stats: &steamlens_core::UserStats<'_>, entry: &StatEntry) -> StatValue {
    match entry.kind {
        StatKind::Int => match stats.get_stat_int(&entry.name) {
            Ok(v) => StatValue::Int(v),
            Err(e) => StatValue::Error(e.to_string()),
        },
        StatKind::Float => match stats.get_stat_float(&entry.name) {
            Ok(v) => StatValue::Float(v),
            Err(e) => StatValue::Error(e.to_string()),
        },
    }
}

// ── Test ──────────────────────────────────────────────────────────────────────

#[test]
#[allow(non_snake_case)]
#[ignore = "DESTRUCTIVE: wipes all achievements and stats for SteamAppId. \
            Requires STEAMLENS_DESTRUCTIVE_OK=yes AND SteamAppId env vars. \
            Purpose: diagnose whether ResetAllStats(true) zeroes stat counters."]
fn diagnose_stat_reset_after_reset_all_stats() {
    if std::env::var("STEAMLENS_DESTRUCTIVE_OK").as_deref() != Ok("yes") {
        panic!(
            "Refusing to run destructive diagnostic. \
             Set STEAMLENS_DESTRUCTIVE_OK=yes AND SteamAppId=<app> to opt in."
        );
    }

    let app_id = std::env::var("SteamAppId")
        .expect("SteamAppId env var must be set (e.g. SteamAppId=105600 for Terraria)");

    let app_id_u32: u32 = app_id
        .parse()
        .expect("SteamAppId must be a valid u32 (e.g. 105600)");

    println!("=== diagnose_stat_reset: app_id={app_id} ===");

    // ── 1. Read schema from disk ──────────────────────────────────────────────

    let path = schema_path(&app_id);
    println!("Schema path: {}", path.display());

    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "Schema cache file not found at {}: {e}\n\
             Has Steam ever downloaded this app's schema? \
             Start the game once or visit the game page in Steam.",
            path.display()
        )
    });

    let schema = steamlens_vdf::parse(&bytes)
        .unwrap_or_else(|e| panic!("Failed to parse schema binary KV: {e}"));

    let stat_entries = parse_stat_entries(&schema, &app_id);
    println!(
        "Found {} non-achievement stat(s) in schema:",
        stat_entries.len()
    );
    for e in &stat_entries {
        println!("  {:?}  {}", e.kind, e.name);
    }

    if stat_entries.is_empty() {
        println!("No non-achievement stats in schema — nothing to diagnose. Exiting.");
        return;
    }

    // ── 2. Connect and load stats ─────────────────────────────────────────────

    let client = connect(app_id_u32).expect("connect(app_id) must succeed with Steam running");
    let stats = client.user_stats();
    let steam_id = client.steam_id();

    println!("\nsteam_id = {steam_id}");

    stats
        .request_user_stats(steam_id)
        .expect("request_user_stats must not fail when Steam is running");

    println!("RequestUserStats dispatched — waiting for UserStatsReceived...");

    wait_for_user_stats_received(&client, steam_id, Duration::from_secs(10))
        .expect("UserStatsReceived must arrive within 10 seconds");

    println!("UserStatsReceived — stats loaded into local cache.");

    // ── 3. Read BEFORE values ─────────────────────────────────────────────────

    println!("\n--- BEFORE reset ---");
    let before: Vec<StatValue> = stat_entries.iter().map(|e| read_stat(&stats, e)).collect();
    for (entry, val) in stat_entries.iter().zip(before.iter()) {
        let kind_label = match entry.kind {
            StatKind::Int => "int",
            StatKind::Float => "float",
        };
        println!("  BEFORE {kind_label}: {} = {val}", entry.name);
    }

    // ── 4. Reset + store ──────────────────────────────────────────────────────

    println!("\nCalling ResetAllStats(true)...");
    stats
        .reset_all_stats(true)
        .expect("reset_all_stats must not fail after UserStatsReceived");

    println!("Calling store_stats()...");
    stats
        .store_stats()
        .expect("store_stats must not fail after reset_all_stats");

    println!("Waiting for UserStatsStored...");
    let stored = wait_for_user_stats_stored(&client, Duration::from_secs(10))
        .expect("UserStatsStored must arrive within 10 seconds");

    println!("UserStatsStored: result={stored:?}");
    assert!(
        stored.is_ok(),
        "UserStatsStored result must be Ok, got {stored:?}"
    );

    // ── 5. Re-request to refresh local cache ─────────────────────────────────

    println!("\nRe-requesting user stats to refresh local cache...");
    stats
        .request_user_stats(steam_id)
        .expect("second request_user_stats must not fail");

    println!("Waiting for second UserStatsReceived...");
    wait_for_user_stats_received(&client, steam_id, Duration::from_secs(10))
        .expect("second UserStatsReceived must arrive within 10 seconds");

    println!("Local stat cache refreshed.");

    // ── 6. Read AFTER values ──────────────────────────────────────────────────

    println!("\n--- AFTER reset ---");
    let after: Vec<StatValue> = stat_entries.iter().map(|e| read_stat(&stats, e)).collect();
    for (entry, val) in stat_entries.iter().zip(after.iter()) {
        let kind_label = match entry.kind {
            StatKind::Int => "int",
            StatKind::Float => "float",
        };
        println!("  AFTER {kind_label}: {} = {val}", entry.name);
    }

    // ── 7. Diff ───────────────────────────────────────────────────────────────

    println!("\n--- DIFF ---");
    let mut any_nonzero_after = false;
    for ((entry, b), a) in stat_entries.iter().zip(before.iter()).zip(after.iter()) {
        let is_zero = match a {
            StatValue::Int(n) => *n == 0,
            StatValue::Float(f) => *f == 0.0,
            StatValue::Error(_) => false,
        };
        let flag = if is_zero { "OK" } else { "!! NON-ZERO" };
        println!("  DIFF: {} : {b} -> {a}  [{flag}]", entry.name);
        if !is_zero {
            any_nonzero_after = true;
        }
    }

    println!();
    if any_nonzero_after {
        println!(
            "RESULT: At least one stat counter is NON-ZERO after ResetAllStats(true) + \
             StoreStats + re-request.\n\
             This indicates that ResetAllStats does NOT fully zero these counters in \
             Steam's runtime, or that the local cache was not updated correctly.\n\
             See entries marked [!! NON-ZERO] above."
        );
    } else {
        println!(
            "RESULT: All stat counters are zero after ResetAllStats(true) + StoreStats + \
             re-request.\n\
             The reset worked correctly at the Steam API level. If the game UI still \
             shows progress, it is reading from a game-local save file or from \
             IndicateAchievementProgress internal state — not from Steam stats."
        );
    }
}
