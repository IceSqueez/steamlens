//! Integration test for the callback poller infrastructure.
//!
//! All tests here are `#[ignore]`d by default — they require a live Steam
//! client to be running and signed in on the host machine.
//!
//! Manual invocation:
//!
//!     cargo test -p steamlens-core --test poll_callbacks \
//!         -- --ignored --nocapture
//!
//! Expected behaviour with Steam running: the first few poll iterations may
//! return one or more init callbacks (e.g. SteamServersConnected). Later
//! iterations return an empty `Vec`. The test must not panic or error.

use std::time::Duration;

use steamlens_core::{SteamCallback, connect};

#[test]
#[ignore = "requires Steam running; polls callbacks 5 times at 100 ms intervals"]
fn poll_five_iterations_does_not_error() {
    let client = connect().expect("connect() must succeed with Steam running");

    for i in 0..5 {
        let callbacks = client
            .poll_callbacks()
            .unwrap_or_else(|e| panic!("poll_callbacks() failed on iteration {i}: {e}"));

        for cb in &callbacks {
            match cb {
                SteamCallback::UserStatsReceived {
                    game_id,
                    result,
                    user_steam_id,
                } => {
                    println!(
                        "iteration={i} UserStatsReceived game_id={game_id} result={result:?} user={user_steam_id}"
                    );
                }
                SteamCallback::UserStatsStored { game_id, result } => {
                    println!("iteration={i} UserStatsStored game_id={game_id} result={result:?}");
                }
                SteamCallback::Unknown(raw) => {
                    println!(
                        "iteration={i} Unknown id={} size={} bytes",
                        raw.id,
                        raw.payload.len()
                    );
                }
                _ => {}
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "requires Steam running; verifies callback payloads are coherent"]
fn callback_payloads_have_sensible_sizes() {
    let client = connect().expect("connect() must succeed with Steam running");

    for _ in 0..10 {
        let callbacks = client
            .poll_callbacks()
            .expect("poll_callbacks must not error");
        for cb in &callbacks {
            if let SteamCallback::Unknown(raw) = cb {
                assert!(
                    raw.payload.len() < 65536,
                    "callback payload suspiciously large: id={} size={} — likely a bad pointer copy",
                    raw.id,
                    raw.payload.len()
                );
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
