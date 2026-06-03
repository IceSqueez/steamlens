use std::path::PathBuf;

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
    let user_profile = crate::paths::user_profile_path(steamid3);
    let user_library = crate::paths::user_library_path(steamid3);

    if user_profile.exists() || user_library.exists() {
        tracing::trace!(steamid3, "migration: already migrated, skipping");
        return Ok(MigrationOutcome::AlreadyMigrated);
    }

    let legacy_profile = crate::paths::legacy_profile_path();
    let legacy_library = crate::paths::legacy_library_path();

    let has_legacy_profile = legacy_profile.exists();
    let has_legacy_library = legacy_library.exists();

    if !has_legacy_profile && !has_legacy_library {
        let legacy_games_dir = crate::paths::cache_dir().join("games");
        if !has_any_legacy_game_data(&legacy_games_dir).await {
            tracing::trace!(steamid3, "migration: no legacy data found");
            return Ok(MigrationOutcome::NothingToMigrate);
        }
    }

    tracing::info!(steamid3, "migration: starting legacy cache migration");

    let user_dir = crate::paths::user_dir(steamid3);
    tokio::fs::create_dir_all(&user_dir).await?;

    let mut files_moved: u32 = 0;

    if has_legacy_profile {
        files_moved += move_file_logged(&legacy_profile, &user_profile).await;
    }

    if has_legacy_library {
        files_moved += move_file_logged(&legacy_library, &user_library).await;
    }

    let legacy_games_dir = crate::paths::cache_dir().join("games");
    if let Ok(mut entries) = tokio::fs::read_dir(&legacy_games_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            let Ok(app_id) = name.parse::<u32>() else {
                continue;
            };

            let legacy_game_dir = crate::paths::legacy_game_dir(app_id);
            let user_game_dir = crate::paths::user_game_dir(steamid3, app_id);

            for filename in ["summary.json", "achievements.json"] {
                let src = legacy_game_dir.join(filename);
                if !src.exists() {
                    continue;
                }
                let dst = user_game_dir.join(filename);
                if let Some(parent) = dst.parent()
                    && let Err(e) = tokio::fs::create_dir_all(parent).await
                {
                    tracing::warn!(
                        steamid3,
                        app_id,
                        filename,
                        error = %e,
                        "migration: could not create user game dir"
                    );
                    continue;
                }
                files_moved += move_file_logged(&src, &dst).await;
            }
        }
    }

    tracing::info!(
        steamid3,
        files_moved,
        "migration: completed legacy cache migration"
    );

    Ok(MigrationOutcome::Migrated { files_moved })
}

async fn move_file_logged(src: &PathBuf, dst: &PathBuf) -> u32 {
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

async fn has_any_legacy_game_data(games_dir: &PathBuf) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(games_dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if name.parse::<u32>().is_ok() {
            let game_dir = games_dir.join(name.as_ref());
            if game_dir.join("summary.json").exists() || game_dir.join("achievements.json").exists()
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STEAMID3: u32 = 123456789;

    fn write_file(path: &std::path::Path, content: &[u8]) {
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
        write_file(&cache.join("games/570/achievements.json"), b"achievements");
        write_file(&cache.join("games/570/icons/foo.png"), b"icon");

        let outcome = migrate_with_paths(TEST_STEAMID3, &cache).await.unwrap();

        match outcome {
            MigrationOutcome::Migrated { files_moved } => {
                assert_eq!(
                    files_moved, 4,
                    "must move 4 files (profile, library, summary, achievements)"
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
            user_root.join("games/570/achievements.json").exists(),
            "achievements moved"
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
    async fn migration_is_idempotent() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = dir.path().join("cache");

        write_file(&cache.join("profile.json"), b"profile");
        write_file(&cache.join("library.json"), b"library");

        let first = migrate_with_paths(TEST_STEAMID3, &cache).await.unwrap();
        assert!(matches!(first, MigrationOutcome::Migrated { .. }));

        let second = migrate_with_paths(TEST_STEAMID3, &cache).await.unwrap();
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

        let outcome = migrate_with_paths(TEST_STEAMID3, &cache).await.unwrap();
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

        let outcome = migrate_with_paths(TEST_STEAMID3, &cache).await.unwrap();
        assert_eq!(outcome, MigrationOutcome::NothingToMigrate);
    }

    async fn migrate_with_paths(
        steamid3: u32,
        cache_root: &std::path::Path,
    ) -> Result<MigrationOutcome, MigrationError> {
        let user_profile = cache_root
            .join("users")
            .join(steamid3.to_string())
            .join("profile.json");
        let user_library = cache_root
            .join("users")
            .join(steamid3.to_string())
            .join("library.json");

        if user_profile.exists() || user_library.exists() {
            return Ok(MigrationOutcome::AlreadyMigrated);
        }

        let legacy_profile = cache_root.join("profile.json");
        let legacy_library = cache_root.join("library.json");
        let legacy_games_dir = cache_root.join("games");

        let has_legacy_profile = legacy_profile.exists();
        let has_legacy_library = legacy_library.exists();

        if !has_legacy_profile && !has_legacy_library {
            let games_dir_path = legacy_games_dir.to_path_buf();
            if !has_any_legacy_game_data(&games_dir_path).await {
                return Ok(MigrationOutcome::NothingToMigrate);
            }
        }

        let user_dir = cache_root.join("users").join(steamid3.to_string());
        tokio::fs::create_dir_all(&user_dir).await?;

        let mut files_moved: u32 = 0;

        if has_legacy_profile {
            files_moved += move_file_logged(&legacy_profile.to_path_buf(), &user_profile).await;
        }

        if has_legacy_library {
            files_moved += move_file_logged(&legacy_library.to_path_buf(), &user_library).await;
        }

        if let Ok(mut entries) = tokio::fs::read_dir(&legacy_games_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let fname = entry.file_name();
                let name = fname.to_string_lossy();
                let Ok(app_id) = name.parse::<u32>() else {
                    continue;
                };

                let legacy_game_dir = legacy_games_dir.join(name.as_ref());
                let user_game_dir = cache_root
                    .join("users")
                    .join(steamid3.to_string())
                    .join("games")
                    .join(app_id.to_string());

                for filename in ["summary.json", "achievements.json"] {
                    let src = legacy_game_dir.join(filename);
                    if !src.exists() {
                        continue;
                    }
                    let dst = user_game_dir.join(filename);
                    if let Some(parent) = dst.parent()
                        && let Err(e) = tokio::fs::create_dir_all(parent).await
                    {
                        tracing::warn!(
                            error = %e,
                            "migration test: could not create user game dir"
                        );
                        continue;
                    }
                    files_moved += move_file_logged(&src.to_path_buf(), &dst.to_path_buf()).await;
                }
            }
        }

        Ok(MigrationOutcome::Migrated { files_moved })
    }
}
