//! Pure unit-style tests that exercise the public surface of `SteamError`
//! without touching Steam. These run on CI.

use std::path::PathBuf;

use steamlens_core::SteamError;

#[test]
fn steam_not_running_renders_user_facing_message() {
    let msg = SteamError::SteamNotRunning.to_string();
    assert!(
        msg.contains("Steam"),
        "SteamNotRunning Display must mention Steam, got: {msg}"
    );
    // The message is shown in the UI; it must not contain raw type names
    // or debug clutter.
    assert!(!msg.contains("SteamError::"));
    assert!(!msg.contains("{"));
}

#[test]
fn install_not_found_lists_searched_paths() {
    let err = SteamError::SteamInstallNotFound {
        searched: vec![
            PathBuf::from("/tmp/fake/path/one"),
            PathBuf::from("/tmp/fake/path/two"),
        ],
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/fake/path/one"));
    assert!(msg.contains("/tmp/fake/path/two"));
}

#[test]
fn install_not_found_with_empty_search_list_does_not_panic() {
    let err = SteamError::SteamInstallNotFound { searched: vec![] };
    // The Display impl must not panic when the slice is empty (regression
    // guard for any future refactor that does `paths[0]`).
    let _ = err.to_string();
}

#[test]
fn errors_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SteamError>();
}

#[test]
fn errors_implement_std_error() {
    fn assert_error<T: std::error::Error + 'static>() {}
    assert_error::<SteamError>();
}
