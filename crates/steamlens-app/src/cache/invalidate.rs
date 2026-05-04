use std::path::{Path, PathBuf};
use std::time::SystemTime;

use steamlens_core::{GameSummary, read_last_played, read_manifest_state};

use crate::cache::{
    CURRENT_SCHEMA_VERSION, CacheHit, GameCacheEntry, store::load_game_cache_from_path,
};
use crate::settings::steamlens_root;

/// Result of the boot-time cache classification pass.
#[derive(Debug, Clone, Default)]
pub struct ClassifyResult {
    /// Games whose cache entry is still valid — no IPC scan needed.
    pub hits: Vec<CacheHit>,
    /// App IDs that require a fresh IPC scan (dirty: new game, stale manifest,
    /// recently played, or schema version bumped).
    pub dirty: Vec<u32>,
    /// Number of cache files discarded because their `schema_version` did not
    /// match `CURRENT_SCHEMA_VERSION`.  Used to show a one-time toast on
    /// upgrades.
    pub schema_bumped: u32,
}

fn cache_root() -> PathBuf {
    steamlens_root().join("cache").join("games")
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Classifies each game as a cache hit or dirty, applying Rule M and Rule R.
///
/// **Rule M (RFC-002 §7.1):** if the appmanifest is missing (game uninstalled)
/// and a cache entry exists, the game is classified as a **hit**, not dirty.
/// An uninstalled game cannot have been played since the last cache write.
///
/// This function is `async` and must be called from within a tokio runtime.
/// Wrap it in `Task::perform` so it does not block the UI thread.
pub async fn classify_games(
    games: &[GameSummary],
    steam_root: &Path,
    steamid3: u64,
) -> ClassifyResult {
    classify_games_with_root(games, steam_root, steamid3, &cache_root()).await
}

/// Testable variant that accepts an explicit cache root instead of the
/// platform default.  Call this from tests using a `TempDir`.
pub(crate) async fn classify_games_with_root(
    games: &[GameSummary],
    steam_root: &Path,
    steamid3: u64,
    cache_root: &Path,
) -> ClassifyResult {
    let mut result = ClassifyResult::default();

    // TODO(perf): parse localconfig once per boot and look up per game instead
    // of re-parsing the ~500 KB file for every game.

    for game in games {
        let app_id = game.app_id;
        let cache_path = cache_root.join(format!("{app_id}.json"));

        let schema_version_from_file = peek_schema_version(&cache_path).await;
        if schema_version_from_file == Some(0) {
            result.dirty.push(app_id);
            continue;
        }

        if let Some(v) = schema_version_from_file
            && v != CURRENT_SCHEMA_VERSION
        {
            result.schema_bumped += 1;
            result.dirty.push(app_id);
            continue;
        }

        let Some(entry) = load_game_cache_from_path(&cache_path).await else {
            result.dirty.push(app_id);
            continue;
        };

        let manifest_state = read_manifest_state(&game.manifest_path);

        match manifest_state {
            None => {
                result.hits.push(CacheHit { app_id, entry });
                continue;
            }
            Some(ms) => {
                if ms.last_updated != entry.steam_last_updated {
                    result.dirty.push(app_id);
                    continue;
                }
            }
        }

        let last_played = read_last_played(steam_root, steamid3, app_id);
        if let Some(lp) = last_played
            && lp > entry.cached_at
        {
            result.dirty.push(app_id);
            continue;
        }

        result.hits.push(CacheHit { app_id, entry });
    }

    result
}

async fn peek_schema_version(cache_path: &Path) -> Option<u32> {
    let bytes = tokio::fs::read(cache_path).await.ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let v = value.get("schema_version")?.as_u64()? as u32;
    Some(v)
}

/// Build a minimal `GameCacheEntry` from a `ProgressScanner` result.
///
/// Called after `ProgressFetched` arrives so the next boot can avoid re-running
/// the IPC scan for this game.  Achievements and stats are left empty (the
/// scanner only fetches counts); Manager open will write a full entry on close.
pub fn make_progress_cache_entry(
    game: &GameSummary,
    earned: u32,
    total: u32,
    steam_root: &Path,
    steamid3: u64,
) -> GameCacheEntry {
    use crate::cache::types::CachedProgress;

    let steam_last_played = read_last_played(steam_root, steamid3, game.app_id).unwrap_or(0);

    GameCacheEntry {
        schema_version: CURRENT_SCHEMA_VERSION,
        app_id: game.app_id,
        name: game.name.clone(),
        steam_last_updated: game.last_updated,
        steam_last_played,
        cached_at: now_epoch(),
        achievements: Vec::new(),
        stats: Vec::new(),
        progress: CachedProgress { earned, total },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::store::atomic_write;
    use crate::cache::types::{CURRENT_SCHEMA_VERSION, CachedProgress, GameCacheEntry};
    use std::path::PathBuf;

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_invalidate_test_{}_{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn make_summary(app_id: u32, manifest_path: PathBuf) -> GameSummary {
        GameSummary {
            app_id,
            name: format!("Game {app_id}"),
            last_played: None,
            achievement_count: 1,
            last_updated: 1_000_000,
            manifest_path,
        }
    }

    async fn write_cache(dir: &Path, app_id: u32, entry: &GameCacheEntry) {
        let path = dir.join(format!("{app_id}.json"));
        let bytes = serde_json::to_vec_pretty(entry).unwrap();
        atomic_write(&path, &bytes).await.unwrap();
    }

    fn make_entry(app_id: u32, last_updated: u64, cached_at: u64) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            steam_last_updated: last_updated,
            steam_last_played: 0,
            cached_at,
            achievements: Vec::new(),
            stats: Vec::new(),
            progress: CachedProgress {
                earned: 1,
                total: 10,
            },
        }
    }

    fn write_manifest(dir: &Path, app_id: u32, last_updated: u64) -> PathBuf {
        let path = dir.join(format!("appmanifest_{app_id}.acf"));
        let content = format!(
            "\"AppState\"\n{{\n    \"appid\" \"{app_id}\"\n    \"LastUpdated\" \"{last_updated}\"\n    \"buildid\" \"1\"\n}}\n"
        );
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn empty_games_list_produces_empty_result() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let result = classify_games_with_root(&[], &steam_root, 123, &cache_dir).await;
        assert!(result.hits.is_empty());
        assert!(result.dirty.is_empty());
        assert_eq!(result.schema_bumped, 0);
    }

    #[tokio::test]
    async fn no_cache_file_goes_dirty() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let manifest = steam_root.join("appmanifest_1.acf");
        let game = make_summary(1, manifest);

        let result = classify_games_with_root(&[game], &steam_root, 0, &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![1]);
        assert_eq!(result.schema_bumped, 0);
    }

    #[tokio::test]
    async fn cache_present_manifest_matches_no_last_played_is_hit() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let last_updated: u64 = 1_000_000;
        let manifest_path = write_manifest(&steam_root, 2, last_updated);
        let mut game = make_summary(2, manifest_path);
        game.last_updated = last_updated;

        let entry = make_entry(2, last_updated, 999_999_999);
        write_cache(&cache_dir, 2, &entry).await;

        let result = classify_games_with_root(&[game], &steam_root, 0, &cache_dir).await;
        assert_eq!(result.hits.len(), 1, "should be a cache hit");
        assert!(result.dirty.is_empty());
        assert_eq!(result.hits[0].app_id, 2);
    }

    #[tokio::test]
    async fn manifest_last_updated_differs_goes_dirty() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let manifest_path = write_manifest(&steam_root, 3, 2_000_000);
        let mut game = make_summary(3, manifest_path);
        game.last_updated = 1_000_000;

        let entry = make_entry(3, 1_000_000, 999_999_999);
        write_cache(&cache_dir, 3, &entry).await;

        let result = classify_games_with_root(&[game], &steam_root, 0, &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![3]);
    }

    #[tokio::test]
    async fn last_played_newer_than_cached_at_goes_dirty() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let last_updated: u64 = 1_000_000;
        let cached_at: u64 = 500;

        let manifest_path = write_manifest(&steam_root, 4, last_updated);
        let mut game = make_summary(4, manifest_path);
        game.last_updated = last_updated;

        let entry = make_entry(4, last_updated, cached_at);
        write_cache(&cache_dir, 4, &entry).await;

        let userdata_dir = steam_root.join("userdata").join("99").join("config");
        std::fs::create_dir_all(&userdata_dir).unwrap();
        let localconfig = "\"UserLocalConfigStore\"\n{\n    \"Software\"\n    {\n        \"Valve\"\n        {\n            \"Steam\"\n            {\n                \"apps\"\n                {\n                    \"4\"\n                    {\n                        \"LastPlayed\" \"1000\"\n                    }\n                }\n            }\n        }\n    }\n}\n";
        std::fs::write(userdata_dir.join("localconfig.vdf"), localconfig).unwrap();

        let result = classify_games_with_root(&[game], &steam_root, 99, &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![4]);
    }

    #[tokio::test]
    async fn rule_m_manifest_missing_cache_present_is_hit() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let nonexistent_manifest = steam_root.join("appmanifest_5_missing.acf");
        let game = make_summary(5, nonexistent_manifest);

        let entry = make_entry(5, 1_000_000, 999_999_999);
        write_cache(&cache_dir, 5, &entry).await;

        let result = classify_games_with_root(&[game], &steam_root, 0, &cache_dir).await;
        assert_eq!(
            result.hits.len(),
            1,
            "Rule M: uninstalled game (missing manifest) with valid cache must be a hit"
        );
        assert!(result.dirty.is_empty(), "Rule M: must NOT be dirty");
        assert_eq!(result.hits[0].app_id, 5);
    }

    #[tokio::test]
    async fn schema_version_mismatch_goes_dirty_with_count() {
        let steam_root = tempdir();
        let cache_dir = tempdir();
        let manifest_path = steam_root.join("appmanifest_6.acf");
        let game = make_summary(6, manifest_path);

        let bad_cache = cache_dir.join("6.json");
        let bad_json = r#"{"schema_version":99,"app_id":6,"name":"Game 6","steam_last_updated":0,"steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], &steam_root, 0, &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![6]);
        assert_eq!(result.schema_bumped, 1);
    }
}
