use std::path::{Path, PathBuf};

use steamlens_core::GameSummary;

use crate::cache::{CURRENT_SCHEMA_VERSION, CacheHit, store::load_game_cache_from_path};
use crate::settings::steamlens_root;

#[derive(Debug, Clone, Default)]
pub struct ClassifyResult {
    pub hits: Vec<CacheHit>,
    pub dirty: Vec<u32>,
    /// Count of cache files discarded because their `schema_version`
    /// did not match `CURRENT_SCHEMA_VERSION`; drives the upgrade toast.
    pub schema_bumped: u32,
}

fn cache_root() -> PathBuf {
    steamlens_root().join("cache").join("games")
}

pub async fn classify_games(
    games: &[GameSummary],
    _steam_root: &Path,
    _steamid3: u64,
) -> ClassifyResult {
    classify_games_with_root(games, &cache_root()).await
}

pub(crate) async fn classify_games_with_root(
    games: &[GameSummary],
    cache_root: &Path,
) -> ClassifyResult {
    let mut result = ClassifyResult::default();

    for game in games {
        let app_id = game.app_id;
        let cache_path = cache_root.join(format!("{app_id}.json"));

        let schema_version_from_file = peek_schema_version(&cache_path).await;
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

        if let Some(lp) = game.last_played
            && (lp as u64) > entry.cached_at
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

    fn make_summary(app_id: u32, last_played: Option<u32>) -> GameSummary {
        GameSummary {
            app_id,
            name: format!("Game {app_id}"),
            last_played,
            achievement_count: 1,
            change_number: 0,
        }
    }

    async fn write_cache(dir: &Path, app_id: u32, entry: &GameCacheEntry) {
        let path = dir.join(format!("{app_id}.json"));
        let bytes = serde_json::to_vec_pretty(entry).unwrap();
        atomic_write(&path, &bytes).await.unwrap();
    }

    fn make_entry(app_id: u32, cached_at: u64) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            steam_last_updated: 0,
            steam_last_played: 0,
            cached_at,
            achievements: Vec::new(),
            stats: Vec::new(),
            progress: CachedProgress {
                earned: 1,
                total: 10,
            },
            tier_breakdown: Vec::new(),
        }
    }

    #[tokio::test]
    async fn empty_games_list_produces_empty_result() {
        let cache_dir = tempdir();
        let result = classify_games_with_root(&[], &cache_dir).await;
        assert!(result.hits.is_empty());
        assert!(result.dirty.is_empty());
        assert_eq!(result.schema_bumped, 0);
    }

    #[tokio::test]
    async fn no_cache_file_goes_dirty() {
        let cache_dir = tempdir();
        let game = make_summary(1, None);

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![1]);
        assert_eq!(result.schema_bumped, 0);
    }

    #[tokio::test]
    async fn cache_present_no_last_played_is_hit() {
        let cache_dir = tempdir();
        let game = make_summary(2, None);

        let entry = make_entry(2, 999_999_999);
        write_cache(&cache_dir, 2, &entry).await;

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert_eq!(result.hits.len(), 1, "should be a cache hit");
        assert!(result.dirty.is_empty());
        assert_eq!(result.hits[0].app_id, 2);
    }

    #[tokio::test]
    async fn last_played_newer_than_cached_at_goes_dirty() {
        let cache_dir = tempdir();
        let cached_at: u64 = 500;
        let game = make_summary(4, Some(1000));

        let entry = make_entry(4, cached_at);
        write_cache(&cache_dir, 4, &entry).await;

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![4]);
    }

    #[tokio::test]
    async fn cache_present_last_played_older_than_cached_at_is_hit() {
        let cache_dir = tempdir();
        let game = make_summary(5, Some(100));

        let entry = make_entry(5, 999_999_999);
        write_cache(&cache_dir, 5, &entry).await;

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert_eq!(
            result.hits.len(),
            1,
            "game not played since last cache write must be a hit"
        );
        assert!(result.dirty.is_empty());
        assert_eq!(result.hits[0].app_id, 5);
    }

    #[tokio::test]
    async fn schema_version_mismatch_goes_dirty_with_count() {
        let cache_dir = tempdir();
        let game = make_summary(6, None);

        let bad_cache = cache_dir.join("6.json");
        let bad_json = r#"{"schema_version":99,"app_id":6,"name":"Game 6","steam_last_updated":0,"steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![6]);
        assert_eq!(result.schema_bumped, 1);
    }

    #[tokio::test]
    async fn schema_version_zero_goes_dirty_and_increments_schema_bumped() {
        let cache_dir = tempdir();
        let game = make_summary(7, None);

        let bad_cache = cache_dir.join("7.json");
        let bad_json = r#"{"schema_version":0,"app_id":7,"name":"Game 7","steam_last_updated":0,"steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], &cache_dir).await;
        assert!(result.hits.is_empty(), "version-0 cache must not be a hit");
        assert_eq!(result.dirty, vec![7], "version-0 cache must be dirty");
        assert_eq!(
            result.schema_bumped, 1,
            "schema_version=0 must increment schema_bumped like any other mismatch"
        );
    }
}
