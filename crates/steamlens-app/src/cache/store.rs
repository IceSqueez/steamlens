use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cache::types::{
    CURRENT_SCHEMA_VERSION, GameAchievementsCache, GameCacheEntry, GameSummaryCache,
    SUMMARY_SCHEMA_VERSION,
};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub enum CacheIoError {
    #[error("I/O error writing cache: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization failed: {0}")]
    Serialize(String),
}

/// Writes `bytes` to a per-call unique `<path>.tmp.<pid>.<seq>` file, fsyncs,
/// then renames over `path`. Atomic on POSIX. Unique tmp names allow concurrent
/// writers to the same target without ENOENT racing on the rename step;
/// last-writer-wins semantics for the final file.
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

fn game_cache_path(app_id: u32) -> PathBuf {
    crate::paths::cache_dir()
        .join("games")
        .join(format!("{app_id}.json"))
}

pub fn load_game_cache_blocking(app_id: u32) -> Option<GameCacheEntry> {
    let path = game_cache_path(app_id);
    let bytes = std::fs::read(&path).ok()?;
    let entry: GameCacheEntry = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!("cache: JSON parse error at {}: {e}", path.display());
        })
        .ok()?;
    if entry.schema_version != CURRENT_SCHEMA_VERSION {
        tracing::warn!(
            "cache: schema version {} != expected {} at {}; treating as cache miss",
            entry.schema_version,
            CURRENT_SCHEMA_VERSION,
            path.display()
        );
        return None;
    }
    Some(entry)
}

#[allow(
    dead_code,
    reason = "retained for rollback safety per cache migration plan; classify uses load_game_summary_from_path"
)]
pub(crate) async fn load_game_cache_from_path(path: &Path) -> Option<GameCacheEntry> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: GameCacheEntry = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!("cache: JSON parse error at {}: {e}", path.display());
        })
        .ok()?;
    if entry.schema_version != CURRENT_SCHEMA_VERSION {
        tracing::warn!(
            "cache: schema version {} != expected {}; treating as cache miss",
            entry.schema_version,
            CURRENT_SCHEMA_VERSION
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

pub(crate) async fn load_game_summary_from_path(path: &Path) -> Option<GameSummaryCache> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: GameSummaryCache = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!("cache: summary JSON parse error at {}: {e}", path.display());
        })
        .ok()?;
    if entry.schema_version != SUMMARY_SCHEMA_VERSION {
        tracing::warn!(
            "cache: summary schema version {} != expected {} at {}; treating as cache miss",
            entry.schema_version,
            SUMMARY_SCHEMA_VERSION,
            path.display()
        );
        return None;
    }
    Some(entry)
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub async fn load_game_summary(app_id: u32) -> Option<GameSummaryCache> {
    load_game_summary_from_path(&crate::paths::game_summary_path(app_id)).await
}

pub async fn write_game_summary(entry: &GameSummaryCache) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(entry).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    let path = crate::paths::game_summary_path(entry.app_id);
    atomic_write(&path, &bytes).await
}

pub async fn delete_game_cache_dir(app_id: u32) -> Result<(), CacheIoError> {
    let dir = crate::paths::cache_dir()
        .join("games")
        .join(app_id.to_string());
    match tokio::fs::remove_dir_all(&dir).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CacheIoError::Io(e)),
    }
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub async fn load_game_achievements(app_id: u32) -> Option<GameAchievementsCache> {
    let path = crate::paths::game_achievements_path(app_id);
    let bytes = tokio::fs::read(&path).await.ok()?;
    let entry: GameAchievementsCache = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!(
                "cache: achievements JSON parse error at {}: {e}",
                path.display()
            );
        })
        .ok()?;
    if entry.schema_version != SUMMARY_SCHEMA_VERSION {
        tracing::warn!(
            "cache: achievements schema version {} != expected {}; treating as cache miss",
            entry.schema_version,
            SUMMARY_SCHEMA_VERSION
        );
        return None;
    }
    Some(entry)
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub async fn write_game_achievements(entry: &GameAchievementsCache) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(entry).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    let path = crate::paths::game_achievements_path(entry.app_id);
    atomic_write(&path, &bytes).await
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let pid = std::process::id();
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".tmp.{pid}.{seq}"));
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, CachedStat, CachedStatValue};

    fn make_entry(app_id: u32, name: &str) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: name.to_owned(),
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
                value: CachedStatValue::Int(42),
                max_value: None,
                min_value: None,
                default_value: None,
                is_increment_only: false,
                permission: 0,
            }],
            progress: CachedProgress {
                earned: 1,
                total: 2,
            },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        }
    }

    fn count_tmp_leftovers(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .expect("readdir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .count()
    }

    #[tokio::test]
    async fn atomic_write_creates_file_with_expected_bytes() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("test.bin");
        let payload = b"hello steamlens";

        atomic_write(&target, payload).await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, payload);

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* files must remain after rename"
        );
    }

    #[tokio::test]
    async fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("overwrite.bin");
        std::fs::write(&target, b"old content").expect("setup");

        atomic_write(&target, b"new content").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"new content");

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* files must remain"
        );
    }

    #[tokio::test]
    async fn atomic_write_concurrent_to_same_target_does_not_race() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("contended.bin");

        let mut tasks = Vec::new();
        for i in 0u8..16 {
            let path = target.clone();
            tasks.push(tokio::spawn(async move {
                let payload = vec![i; 64];
                atomic_write(&path, &payload).await
            }));
        }

        for t in tasks {
            t.await
                .expect("join")
                .expect("each concurrent write must succeed");
        }

        let final_bytes = std::fs::read(&target).expect("final read");
        assert_eq!(final_bytes.len(), 64, "final file size must be 64 bytes");
        let marker = final_bytes[0];
        assert!(
            final_bytes.iter().all(|&b| b == marker),
            "final file must be one writer's full payload, not a mix"
        );

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* leftovers from concurrent writes"
        );
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
        assert_eq!(restored.stats[0].value, CachedStatValue::Int(42));
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

        let entry_v2 = make_entry(200, "Game Two");
        let bytes_v2 = serde_json::to_vec_pretty(&entry_v2).expect("serialize v2");
        atomic_write(&path, &bytes_v2).await.expect("write v2");

        let loaded = load_game_cache_from_path(&path)
            .await
            .expect("should deserialize v2");
        assert_eq!(loaded.name, "Game Two");
    }

    #[tokio::test]
    async fn load_game_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("999.json");
        let bad_json = br#"{"schema_version":999,"app_id":999,"name":"Bad","steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
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

    #[tokio::test]
    async fn game_summary_round_trip() {
        let summary = GameSummaryCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 105600,
            name: "Terraria".to_owned(),
            cached_change_number: 42,
            cached_at: 1_746_360_000,
            progress: CachedProgress {
                earned: 18,
                total: 88,
            },
            tier_breakdown: Vec::new(),
            genre: Some("Action".to_owned()),
            playtime_minutes: None,
        };

        let bytes = serde_json::to_vec_pretty(&summary).expect("serialize");
        let restored: GameSummaryCache = serde_json::from_slice(&bytes).expect("deserialize");

        assert_eq!(restored.app_id, summary.app_id);
        assert_eq!(restored.cached_change_number, 42);
        assert_eq!(restored.progress.earned, 18);
        assert_eq!(restored.progress.total, 88);
        assert_eq!(restored.genre.as_deref(), Some("Action"));
        assert_eq!(restored.schema_version, SUMMARY_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn game_achievements_round_trip() {
        let achievements = GameAchievementsCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 105600,
            cached_at: 1_746_360_000,
            achievements: vec![CachedAchievement {
                api_name: "KILL_BOSS".to_owned(),
                display_name: "Miner for Fire".to_owned(),
                description: "Defeat the Wall of Flesh.".to_owned(),
                hidden: false,
                icon_path: None,
                icon_locked_path: None,
                earned: true,
                earned_at: Some(1_700_000_000),
                global_percent: Some(18.5),
            }],
            stats: vec![CachedStat {
                api_name: "NumDeaths".to_owned(),
                display_name: "Deaths".to_owned(),
                value: CachedStatValue::Int(42),
                max_value: None,
                min_value: None,
                default_value: None,
                is_increment_only: false,
                permission: 0,
            }],
        };

        let bytes = serde_json::to_vec_pretty(&achievements).expect("serialize");
        let restored: GameAchievementsCache = serde_json::from_slice(&bytes).expect("deserialize");

        assert_eq!(restored.app_id, achievements.app_id);
        assert_eq!(restored.achievements.len(), 1);
        assert_eq!(restored.achievements[0].api_name, "KILL_BOSS");
        assert!(restored.achievements[0].earned);
        assert_eq!(restored.stats[0].value, CachedStatValue::Int(42));
        assert_eq!(restored.schema_version, SUMMARY_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn write_and_load_game_summary_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("summary.json");

        let summary = GameSummaryCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 200,
            name: "Half-Life 2".to_owned(),
            cached_change_number: 7,
            cached_at: 1_746_000_000,
            progress: CachedProgress {
                earned: 10,
                total: 33,
            },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        };

        let bytes = serde_json::to_vec_pretty(&summary).expect("serialize");
        atomic_write(&path, &bytes).await.expect("write");

        let read_back = std::fs::read(&path).expect("read");
        let restored: GameSummaryCache = serde_json::from_slice(&read_back).expect("deserialize");
        assert_eq!(restored.cached_change_number, 7);
        assert_eq!(restored.progress.total, 33);
    }
}
