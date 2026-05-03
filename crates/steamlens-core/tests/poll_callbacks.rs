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
//! return one or more init callbacks (e.g. id=1701 SteamServersConnected).
//! Later iterations return an empty `Vec`. The test must not panic or error.

use std::time::Duration;

use steamlens_core::connect;

#[test]
#[ignore = "requires Steam running; polls callbacks 5 times at 100 ms intervals"]
fn poll_five_iterations_does_not_error() {
    let client = connect().expect("connect() must succeed with Steam running");

    for i in 0..5 {
        let callbacks = client
            .poll_callbacks()
            .unwrap_or_else(|e| panic!("poll_callbacks() failed on iteration {i}: {e}"));

        for cb in &callbacks {
            println!("iteration={i} id={} size={} bytes", cb.id, cb.payload.len());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "requires Steam running; verifies RawCallback fields are coherent"]
fn callback_payloads_have_sensible_sizes() {
    let client = connect().expect("connect() must succeed with Steam running");

    for _ in 0..10 {
        let callbacks = client
            .poll_callbacks()
            .expect("poll_callbacks must not error");
        for cb in &callbacks {
            assert!(
                cb.payload.len() < 65536,
                "callback payload suspiciously large: id={} size={} — likely a bad pointer copy",
                cb.id,
                cb.payload.len()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
