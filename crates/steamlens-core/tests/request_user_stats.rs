// Integration test for RequestUserStats + UserStatsReceived callback.
//
// Requires a live Steam client signed in on the host machine. Run with:
//
//   SteamAppId=480 cargo test -p steamlens-core --test request_user_stats \
//       -- --ignored --nocapture
//
// SteamAppId=480 is Spacewar — a free Valve test app that every Steam account
// owns and that has a populated stats schema. You can also omit SteamAppId to
// use whatever context the current Steam session has active.
//
// If SteamAppId is set, the test additionally asserts that the game_id in the
// UserStatsReceived callback matches it. If the env var is absent the assertion
// is skipped (a warning is printed instead).
//
// Expected output: within 5 seconds the test prints a UserStatsReceived line
// and asserts result.is_ok(). If Steam does not deliver the callback within
// the poll window the test panics with a clear timeout message.

use std::time::Duration;

use steamlens_core::{SteamCallback, connect};

#[test]
#[ignore = "requires Steam running; calls RequestUserStats and waits for callback"]
fn request_user_stats_delivers_callback() {
    let client = connect().expect("connect() must succeed with Steam running");
    let steam_id = client.steam_id();

    let expected_game_id: Option<u64> = std::env::var("SteamAppId")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());

    if expected_game_id.is_none() {
        println!("warning: SteamAppId not set — game_id assertion will be skipped");
    }

    println!("steam_id = {steam_id}");

    let stats = client.user_stats();
    stats
        .request_user_stats(steam_id)
        .expect("request_user_stats must not fail when Steam is running");

    println!("RequestUserStats dispatched — polling for UserStatsReceived...");

    let max_iterations = 50;
    let poll_interval = Duration::from_millis(100);

    for i in 0..max_iterations {
        let callbacks = client
            .poll_callbacks()
            .unwrap_or_else(|e| panic!("poll_callbacks failed on iteration {i}: {e}"));

        for cb in &callbacks {
            match cb {
                SteamCallback::UserStatsReceived {
                    game_id,
                    result,
                    user_steam_id,
                } => {
                    println!(
                        "iteration={i} UserStatsReceived game_id={game_id} result={result:?} user_steam_id={user_steam_id}"
                    );

                    assert_eq!(
                        *user_steam_id, steam_id,
                        "UserStatsReceived user_steam_id must match the connected user"
                    );
                    assert!(
                        result.is_ok(),
                        "UserStatsReceived result must be Ok, got {result:?}"
                    );

                    if let Some(expected) = expected_game_id {
                        assert_eq!(
                            *game_id, expected,
                            "UserStatsReceived game_id={game_id} does not match SteamAppId={expected}"
                        );
                    }

                    return;
                }
                SteamCallback::UserStatsStored { game_id, result } => {
                    println!("iteration={i} UserStatsStored game_id={game_id} result={result:?}");
                }
                SteamCallback::Unknown(raw) => {
                    println!(
                        "iteration={i} Unknown id={} size={}",
                        raw.id,
                        raw.payload.len()
                    );
                }
                _ => {}
            }
        }

        std::thread::sleep(poll_interval);
    }

    panic!(
        "UserStatsReceived not received within {}ms — \
         check that Steam is running and SteamAppId is set to a valid app",
        max_iterations * poll_interval.as_millis()
    );
}
