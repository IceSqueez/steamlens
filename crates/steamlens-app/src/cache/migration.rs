use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOutcome {
    Migrated { files_moved: u32 },
    AlreadyMigrated,
    NothingToMigrate,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("I/O error during cache migration: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn migrate_legacy_cache_if_present(
    steamid3: u32,
) -> Result<MigrationOutcome, MigrationError> {
    migrate_at_root(steamid3, &crate::paths::cache_dir()).await
}

async fn migrate_at_root(
    steamid3: u32,
    cache_root: &Path,
) -> Result<MigrationOutcome, MigrationError> {
    let users_dir = cache_root.join("users").join(steamid3.to_string());
    let user_profile = users_dir.join("profile.json");
    let user_library = users_dir.join("library.json");
    let user_games_dir = users_dir.join("games");

    let legacy_profile = cache_root.join("profile.json");
    let legacy_library = cache_root.join("library.json");
    let legacy_games_dir = cache_root.join("games");

    let had_legacy = legacy_profile.exists()
        || legacy_library.exists()
        || has_any_legacy_game_data(&legacy_games_dir).await;
    let had_target = user_profile.exists() || user_library.exists();

    if !had_legacy {
        return Ok(if had_target {
            MigrationOutcome::AlreadyMigrated
        } else {
            MigrationOutcome::NothingToMigrate
        });
    }

    tracing::info!(steamid3, "migration: starting legacy cache migration");
    tokio::fs::create_dir_all(&users_dir).await?;

    let mut files_moved: u32 = 0;

    files_moved += move_if_pending(&legacy_profile, &user_profile).await;
    files_moved += move_if_pending(&legacy_library, &user_library).await;

    if let Ok(mut entries) = tokio::fs::read_dir(&legacy_games_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();

            if let Some(stem) = name.strip_suffix(".json")
                && let Ok(app_id) = stem.parse::<u32>()
            {
                let src = legacy_games_dir.join(name.as_ref());
                let dst = user_games_dir.join(app_id.to_string()).join("cache.json");
                files_moved += move_if_pending(&src, &dst).await;
                continue;
            }

            if let Ok(app_id) = name.parse::<u32>() {
                let src = legacy_games_dir.join(name.as_ref()).join("summary.json");
                let dst = user_games_dir.join(app_id.to_string()).join("summary.json");
                files_moved += move_if_pending(&src, &dst).await;
            }
        }
    }

    if files_moved == 0 {
        tracing::info!(steamid3, "migration: no new files to move");
        return Ok(MigrationOutcome::AlreadyMigrated);
    }

    tracing::info!(
        steamid3,
        files_moved,
        "migration: completed legacy cache migration"
    );
    Ok(MigrationOutcome::Migrated { files_moved })
}

async fn move_if_pending(src: &Path, dst: &Path) -> u32 {
    if !src.exists() {
        return 0;
    }
    if dst.exists() {
        tracing::warn!(
            src = %src.display(),
            dst = %dst.display(),
            "migration: destination already exists; leaving legacy source as orphan"
        );
        return 0;
    }
    if let Some(parent) = dst.parent()
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        tracing::warn!(
            src = %src.display(),
            error = %e,
            "migration: could not create destination directory"
        );
        return 0;
    }
    match tokio::fs::rename(src, dst).await {
        Ok(()) => {
            tracing::trace!(
                src = %src.display(),
                dst = %dst.display(),
                "migration: moved file"
            );
            1
        }
        Err(e) => {
            tracing::warn!(
                src = %src.display(),
                dst = %dst.display(),
                error = %e,
                "migration: failed to move file; orphan remains at src"
            );
            0
        }
    }
}

async fn has_any_legacy_game_data(games_dir: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(games_dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if let Some(stem) = name.strip_suffix(".json")
            && stem.parse::<u32>().is_ok()
        {
            return true;
        }
        if name.parse::<u32>().is_ok()
            && games_dir.join(name.as_ref()).join("summary.json").exists()
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STEAMID3: u32 = 123456789;

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn migration_moves_per_user_files_and_leaves_icons() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("profile.json"), b"profile");
        write_file(&cache.join("library.json"), b"library");
        write_file(&cache.join("games/570/summary.json"), b"summary");
        write_file(&cache.join("games/570/icons/foo.png"), b"icon");

        let outcome = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();

        match outcome {
            MigrationOutcome::Migrated { files_moved } => {
                assert_eq!(
                    files_moved, 3,
                    "must move 3 files (profile, library, summary)"
                );
            }
            other => panic!("expected Migrated, got {other:?}"),
        }

        let user_root = cache.join("users").join(TEST_STEAMID3.to_string());
        assert!(user_root.join("profile.json").exists(), "profile moved");
        assert!(user_root.join("library.json").exists(), "library moved");
        assert!(
            user_root.join("games/570/summary.json").exists(),
            "summary moved"
        );

        assert!(
            cache.join("games/570/icons/foo.png").exists(),
            "icons must NOT be moved"
        );
        assert!(
            !cache.join("profile.json").exists(),
            "legacy profile must be gone"
        );
        assert!(
            !cache.join("library.json").exists(),
            "legacy library must be gone"
        );
        assert!(
            !cache.join("games/570/summary.json").exists(),
            "legacy summary must be gone"
        );
    }

    #[tokio::test]
    async fn migration_moves_legacy_full_game_cache_files() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("games/570.json"), b"full cache for 570");
        write_file(&cache.join("games/440.json"), b"full cache for 440");
        write_file(&cache.join("games/570/icons/x.png"), b"icon");

        let outcome = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();

        match outcome {
            MigrationOutcome::Migrated { files_moved } => {
                assert_eq!(files_moved, 2, "two cache.json files must be moved");
            }
            other => panic!("expected Migrated, got {other:?}"),
        }

        let user_games = cache
            .join("users")
            .join(TEST_STEAMID3.to_string())
            .join("games");
        assert!(user_games.join("570/cache.json").exists());
        assert!(user_games.join("440/cache.json").exists());
        assert!(!cache.join("games/570.json").exists());
        assert!(!cache.join("games/440.json").exists());
        assert!(
            cache.join("games/570/icons/x.png").exists(),
            "icons must NOT be moved"
        );
    }

    #[tokio::test]
    async fn migration_moves_pending_cache_file_after_prior_migration() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("profile.json"), b"profile");
        let first = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        assert!(matches!(first, MigrationOutcome::Migrated { .. }));

        write_file(&cache.join("games/570.json"), b"latecomer");
        let second = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        match second {
            MigrationOutcome::Migrated { files_moved } => {
                assert_eq!(files_moved, 1, "the new cache.json must be moved");
            }
            other => panic!("expected Migrated, got {other:?}"),
        }

        let user_root = cache.join("users").join(TEST_STEAMID3.to_string());
        assert!(user_root.join("games/570/cache.json").exists());
        assert!(!cache.join("games/570.json").exists());
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("profile.json"), b"profile");
        write_file(&cache.join("library.json"), b"library");

        let first = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        assert!(matches!(first, MigrationOutcome::Migrated { .. }));

        let second = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        assert_eq!(
            second,
            MigrationOutcome::AlreadyMigrated,
            "second call must be no-op"
        );
    }

    #[tokio::test]
    async fn migration_partial_state_profile_only() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("profile.json"), b"profile");

        let outcome = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        match outcome {
            MigrationOutcome::Migrated { files_moved } => {
                assert_eq!(files_moved, 1, "only profile should be moved");
            }
            other => panic!("expected Migrated, got {other:?}"),
        }

        let user_root = cache.join("users").join(TEST_STEAMID3.to_string());
        assert!(user_root.join("profile.json").exists());
        assert!(!user_root.join("library.json").exists());
    }

    #[tokio::test]
    async fn migration_nothing_to_migrate() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();

        let outcome = migrate_at_root(TEST_STEAMID3, &cache).await.unwrap();
        assert_eq!(outcome, MigrationOutcome::NothingToMigrate);
    }
}
