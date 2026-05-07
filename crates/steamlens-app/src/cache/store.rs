use std::path::{Path, PathBuf};

use crate::cache::types::{CURRENT_SCHEMA_VERSION, GameCacheEntry};

#[derive(Debug, thiserror::Error)]
pub enum CacheIoError {
    #[error("I/O error writing cache: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization failed: {0}")]
    Serialize(String),
}

/// `<path>.tmp` is fsync'd then renamed over `path` (atomic on POSIX;
/// same-filesystem `.tmp` avoids cross-mount surprises).
pub async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CacheIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp_path = tmp_path_for(path);

    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.sync_data().await?;
    }

    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}

pub fn game_cache_path(app_id: u32) -> PathBuf {
    crate::settings::steamlens_root()
        .join("cache")
        .join("games")
        .join(format!("{app_id}.json"))
}

pub(crate) async fn load_game_cache_from_path(path: &Path) -> Option<GameCacheEntry> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: GameCacheEntry = serde_json::from_slice(&bytes)
        .map_err(|e| {
            eprintln!(
                "[steamlens] cache: JSON parse error at {}: {e}",
                path.display()
            );
        })
        .ok()?;
    if entry.schema_version != CURRENT_SCHEMA_VERSION {
        eprintln!(
            "[steamlens] cache: schema version {} != expected {}; treating as cache miss",
            entry.schema_version, CURRENT_SCHEMA_VERSION
        );
        return None;
    }
    Some(entry)
}

pub async fn write_game_cache(entry: &GameCacheEntry) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(entry).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    let path = game_cache_path(entry.app_id);
    atomic_write(&path, &bytes).await
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, CachedStat};

    fn make_entry(app_id: u32, name: &str) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: name.to_owned(),
            steam_last_updated: 1_773_063_072,
            steam_last_played: 1_777_926_953,
            cached_at: 1_746_360_000,
            achievements: vec![
                CachedAchievement {
                    api_name: "KILL_BOSS".to_owned(),
                    display_name: "Miner for Fire".to_owned(),
                    description: "Defeat the Wall of Flesh.".to_owned(),
                    hidden: false,
                    icon_path: Some("achievements/105600/KILL_BOSS.jpg".to_owned()),
                    icon_locked_path: Some("achievements/105600/KILL_BOSS_locked.jpg".to_owned()),
                    earned: true,
                    earned_at: Some(1_700_000_000),
                    global_percent: Some(18.5),
                },
                CachedAchievement {
                    api_name: "NEVER_EARNED".to_owned(),
                    display_name: "Hidden Gem".to_owned(),
                    description: String::new(),
                    hidden: true,
                    icon_path: None,
                    icon_locked_path: None,
                    earned: false,
                    earned_at: None,
                    global_percent: None,
                },
            ],
            stats: vec![CachedStat {
                api_name: "NumDeaths".to_owned(),
                display_name: "Deaths".to_owned(),
                value_int: Some(42),
                value_float: None,
                default_value: Some(0.0),
            }],
            progress: CachedProgress {
                earned: 1,
                total: 2,
            },
            tier_breakdown: Vec::new(),
            genre: None,
        }
    }

    #[tokio::test]
    async fn atomic_write_creates_file_with_expected_bytes() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("test.bin");
        let payload = b"hello steamlens";

        atomic_write(&target, payload).await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, payload);

        let tmp = dir.path().join("test.bin.tmp");
        assert!(!tmp.exists(), ".tmp file must not remain after rename");
    }

    #[tokio::test]
    async fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("overwrite.bin");
        std::fs::write(&target, b"old content").expect("setup");

        atomic_write(&target, b"new content").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"new content");

        let tmp = dir.path().join("overwrite.bin.tmp");
        assert!(!tmp.exists(), ".tmp file must not remain");
    }

    #[tokio::test]
    async fn atomic_write_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("nested").join("deep").join("file.bin");

        atomic_write(&target, b"nested").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"nested");
    }

    #[tokio::test]
    async fn game_cache_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("105600.json");
        let original = make_entry(105600, "Terraria");

        let bytes = serde_json::to_vec_pretty(&original).expect("serialize");
        atomic_write(&path, &bytes).await.expect("write");

        let restored = load_game_cache_from_path(&path)
            .await
            .expect("should deserialize");

        assert_eq!(restored.app_id, original.app_id);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.schema_version, original.schema_version);
        assert_eq!(restored.steam_last_updated, original.steam_last_updated);
        assert_eq!(restored.steam_last_played, original.steam_last_played);
        assert_eq!(restored.cached_at, original.cached_at);
        assert_eq!(restored.achievements.len(), original.achievements.len());
        assert_eq!(restored.achievements[0].api_name, "KILL_BOSS");
        assert!(restored.achievements[0].earned);
        assert_eq!(restored.achievements[0].earned_at, Some(1_700_000_000));
        assert_eq!(restored.achievements[0].global_percent, Some(18.5));
        assert_eq!(restored.achievements[1].earned_at, None);
        assert_eq!(restored.achievements[1].global_percent, None);
        assert_eq!(restored.achievements[1].icon_path, None);
        assert_eq!(restored.stats[0].value_int, Some(42));
        assert_eq!(restored.stats[0].value_float, None);
        assert_eq!(restored.progress.earned, 1);
        assert_eq!(restored.progress.total, 2);
    }

    #[tokio::test]
    async fn game_cache_null_options_round_trip() {
        let entry = make_entry(105600, "Terraria");
        let json = serde_json::to_vec_pretty(&entry).expect("serialize");
        let text = std::str::from_utf8(&json).expect("utf8");

        assert!(
            text.contains("\"earned_at\": null"),
            "None earned_at must serialize as null, got:\n{text}"
        );
        assert!(
            text.contains("\"global_percent\": null"),
            "None global_percent must serialize as null, got:\n{text}"
        );
        assert!(
            text.contains("\"icon_path\": null"),
            "None icon_path must serialize as null, got:\n{text}"
        );

        let restored: GameCacheEntry = serde_json::from_slice(&json).expect("deserialize");
        assert_eq!(restored.achievements[1].earned_at, None);
        assert_eq!(restored.achievements[1].global_percent, None);
        assert_eq!(restored.achievements[1].icon_path, None);
    }

    #[tokio::test]
    async fn game_cache_atomic_overwrite() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("200.json");

        let entry_v1 = make_entry(200, "Game One");
        let bytes_v1 = serde_json::to_vec_pretty(&entry_v1).expect("serialize v1");
        atomic_write(&path, &bytes_v1).await.expect("write v1");

        let mut entry_v2 = make_entry(200, "Game Two");
        entry_v2.steam_last_updated = 9_999_999_999;
        let bytes_v2 = serde_json::to_vec_pretty(&entry_v2).expect("serialize v2");
        atomic_write(&path, &bytes_v2).await.expect("write v2");

        let loaded = load_game_cache_from_path(&path)
            .await
            .expect("should deserialize v2");
        assert_eq!(loaded.name, "Game Two");
        assert_eq!(loaded.steam_last_updated, 9_999_999_999);
    }

    #[tokio::test]
    async fn load_game_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("999.json");
        let bad_json = br#"{"schema_version":999,"app_id":999,"name":"Bad","steam_last_updated":0,"steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
        std::fs::write(&path, bad_json).expect("write");

        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "schema mismatch must return None");
    }

    #[tokio::test]
    async fn load_game_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("nonexistent_app.json");
        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "missing file must return None");
    }

    #[tokio::test]
    async fn load_game_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"not json at all ][[[").expect("write");

        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "corrupted JSON must return None");
    }
}
