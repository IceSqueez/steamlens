use steamlens_core::library::scan_with_steam_root;
use steamlens_core::scan_installed_games;

/// Requires a real Steam installation.  Run with:
///   cargo test -p steamlens-core --test library_scan scan_returns_at_least_one_game -- --ignored --nocapture
#[test]
#[ignore]
fn scan_returns_at_least_one_game() {
    let games = scan_installed_games().expect("scan_installed_games failed");
    assert!(
        !games.is_empty(),
        "expected at least one game with achievements in the Steam library"
    );

    println!("Found {} games with achievements:", games.len());
    for g in &games {
        println!(
            "  {:>8}  {:>4} achievements  last_played={:?}  {}",
            g.app_id, g.achievement_count, g.last_played, g.name
        );
    }

    assert!(
        games.iter().all(|g| g.achievement_count > 0),
        "all returned games must have achievement_count > 0"
    );
}

/// Non-ignored: feed an empty temp directory as the Steam root and verify
/// that the scan completes without error and returns an empty list.
#[test]
fn scan_handles_missing_libraryfolders() {
    let tmp = std::env::temp_dir().join(format!(
        "steamlens_lib_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();

    let result = scan_with_steam_root(&tmp);
    assert!(
        result.is_ok(),
        "scan with empty root should not error: {:?}",
        result
    );
    assert!(
        result.unwrap().is_empty(),
        "empty root should yield no games"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Non-ignored: a root that has a libraryfolders.vdf but an entirely empty
/// steamapps directory yields an empty result without error.
#[test]
fn scan_empty_steamapps_dir_yields_no_games() {
    let tmp = std::env::temp_dir().join(format!(
        "steamlens_lib_test2_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    let steamapps = tmp.join("steamapps");
    std::fs::create_dir_all(&steamapps).unwrap();

    std::fs::write(
        steamapps.join("libraryfolders.vdf"),
        r#"
"libraryfolders"
{
    "0"
    {
        "path"  "/nonexistent/path"
    }
}
"#,
    )
    .unwrap();

    let result = scan_with_steam_root(&tmp);
    assert!(result.is_ok());

    std::fs::remove_dir_all(&tmp).ok();
}
