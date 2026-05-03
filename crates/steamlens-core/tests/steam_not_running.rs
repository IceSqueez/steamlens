//! Integration test: `connect()` must surface `SteamError::SteamNotRunning`
//! when the Steam client is not active, instead of panicking.
//!
//! This test is `#[ignore]`d by default because CI does not have a Steam
//! installation, and because the result depends on whether the developer's
//! Steam client is running at the time of execution.
//!
//! Manual invocation:
//!
//!     # Steam OFFLINE — expect SteamNotRunning
//!     cargo test -p steamlens-core --test steam_not_running \
//!         steam_offline_returns_not_running -- --ignored --nocapture
//!
//!     # Steam ONLINE — expect Ok(client) with a non-zero Steam ID
//!     cargo test -p steamlens-core --test steam_not_running \
//!         steam_online_returns_client -- --ignored --nocapture

use steamlens_core::{SteamError, connect};

#[test]
#[ignore = "requires Steam to be NOT running; see file header"]
fn steam_offline_returns_not_running() {
    match connect() {
        Err(SteamError::SteamNotRunning) => {}
        Err(SteamError::SteamInstallNotFound { .. }) => {
            // Acceptable on machines without Steam installed at all — still
            // a typed error, no panic. The point of this test is that we
            // do not panic when Steam is unavailable.
        }
        Err(other) => panic!(
            "expected SteamNotRunning or SteamInstallNotFound when Steam is offline, got: {other:?}"
        ),
        Ok(client) => panic!(
            "expected an error when Steam is offline, but `connect()` succeeded with Steam ID {}",
            client.steam_id()
        ),
    }
}

/// Regression guard for the `Drop`-time SIGSEGV first reproduced on
/// 2026-05-03 and fixed in the same session.
///
/// Before the fix, `connect()` followed by `drop(client)` on a non-main
/// thread crashed with SIGSEGV in `dlclose(steamclient.so)` because Steam's
/// internal worker threads (callback dispatch, IPC reader) still held
/// instruction pointers into the about-to-be-unmapped library text segment.
///
/// The fix: `SteamLibrary` is owned by a process-global `OnceLock` and is
/// never unloaded — `Drop for Library` (which calls `dlclose`) cannot run.
///
/// This test is `#[ignore]`d because it requires a live Steam client.
///
/// Run:
///     cargo test -p steamlens-core --test steam_not_running \
///         drop_on_worker_thread_does_not_segfault -- --ignored --nocapture
#[test]
#[ignore = "requires Steam running; regression guard for drop-time SIGSEGV"]
fn drop_on_worker_thread_does_not_segfault() {
    let handle = std::thread::spawn(|| {
        let client = connect().expect("connect on worker thread");
        let id = client.steam_id();
        assert_ne!(id, 0);
        // implicit drop of `client` here, on the worker thread; before the
        // fix this triggered `Library::Drop` -> `dlclose` -> UB.
    });
    handle.join().expect("worker thread should not panic");
}

#[test]
#[ignore = "requires Steam to be running and signed in; see file header"]
fn steam_online_returns_client() {
    match connect() {
        Ok(client) => {
            assert_ne!(
                client.steam_id(),
                0,
                "Steam returned a zero SteamID — this usually means the ABI \
                 for GetSteamID is wrong on this platform (see STEAM_NOTES.md \
                 'GetSteamID return ABI differs between Linux and Windows')"
            );
            // Sanity-check the SteamID lives in the public-individual range
            // (Universe=Public, AccountType=Individual). Anything outside this
            // range from `connect()` indicates we read garbage.
            let universe = (client.steam_id() >> 56) & 0xFF;
            let account_type = (client.steam_id() >> 52) & 0xF;
            assert_eq!(universe, 1, "Universe byte should be 1 (Public)");
            assert_eq!(
                account_type, 1,
                "AccountType nibble should be 1 (Individual)"
            );
            println!("OK — Steam ID = {}", client.steam_id());
        }
        Err(e) => panic!("expected Ok(Client) with Steam running, got: {e:?}"),
    }
}
