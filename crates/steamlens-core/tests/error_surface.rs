use std::path::PathBuf;

use steamlens_core::SteamError;

#[test]
fn steam_not_running_renders_user_facing_message() {
    let msg = SteamError::SteamNotRunning.to_string();
    assert!(
        msg.contains("Steam"),
        "SteamNotRunning Display must mention Steam, got: {msg}"
    );
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

#[test]
fn invalid_string_display_mentions_nul() {
    let source = std::ffi::CString::new("bad\0name").unwrap_err();
    let err = SteamError::InvalidString { source };
    let msg = err.to_string();
    assert!(
        msg.contains("NUL") || msg.contains("nul") || msg.contains("interior"),
        "InvalidString Display must describe the NUL problem, got: {msg}"
    );
}

#[test]
fn call_failed_display_includes_method_name() {
    let err = SteamError::CallFailed {
        method: "GetAchievement",
    };
    let msg = err.to_string();
    assert!(
        msg.contains("GetAchievement"),
        "CallFailed Display must include the method name, got: {msg}"
    );
}

#[test]
fn achievement_not_found_display_includes_name() {
    let err = SteamError::AchievementNotFound {
        name: "ACH_HIDDEN_BOSS".to_owned(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("ACH_HIDDEN_BOSS"),
        "AchievementNotFound Display must include the name, got: {msg}"
    );
}
