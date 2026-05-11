use std::path::{Path, PathBuf};

use steamlens_core::GameSummary;

use crate::cache::CacheHit;
use crate::cache::store::load_game_summary_from_path;
use crate::paths::cache_dir;

#[derive(Debug, Clone, Default)]
pub struct ClassifyResult {
    pub hits: Vec<CacheHit>,
    pub dirty: Vec<u32>,
    pub schema_bumped: u32,
    pub invalidation_count: u32,
}

fn cache_root() -> PathBuf {
    cache_dir().join("games")
}

pub async fn classify_games(
    game_summaries: &[GameSummary],
    _steam_root: &Path,
    _steamid3: u64,
) -> ClassifyResult {
    classify_games_with_root(game_summaries, &cache_root()).await
}

pub(crate) async fn classify_games_with_root(
    game_summaries: &[GameSummary],
    cache_root: &Path,
) -> ClassifyResult {
    let mut result = ClassifyResult::default();

    for game in game_summaries {
        let app_id = game.app_id;
        let summary_path = cache_root.join(app_id.to_string()).join("summary.json");
        let summary_file_exists = tokio::fs::try_exists(&summary_path).await.unwrap_or(false);

        let Some(summary) = load_game_summary_from_path(&summary_path).await else {
            if summary_file_exists {
                crate::log!(
                    "cache classify: app_id={app_id} summary has bad schema → dirty (schema bump)"
                );
                result.schema_bumped += 1;
            }
            result.dirty.push(app_id);
            result.invalidation_count += 1;
            continue;
        };

        if summary.cached_change_number != game.change_number {
            crate::log!(
                "cache classify: app_id={app_id} change_number changed ({} → {}) → dirty",
                summary.cached_change_number,
                game.change_number
            );
            result.dirty.push(app_id);
            result.invalidation_count += 1;
            continue;
        }

        if let Some(lp) = game.last_played
            && (lp as u64) > summary.cached_at
        {
            crate::log!(
                "cache classify: app_id={app_id} played since cache ({} > {}) → dirty",
                lp,
                summary.cached_at
            );
            result.dirty.push(app_id);
            result.invalidation_count += 1;
            continue;
        }

        let entry_compat = synthesize_compat_entry(summary);
        result.hits.push(CacheHit {
            app_id,
            entry: entry_compat,
        });
    }

    result
}

/// Builds a `GameCacheEntry` from a Layer 1 summary. Achievement and stat
/// vectors are intentionally empty; Layer 2 wiring populates them in a later
/// migration chunk. Callers that only need name + progress see a complete
/// picture; callers that need achievements must load Layer 2 separately.
fn synthesize_compat_entry(
    summary: crate::cache::types::GameSummaryCache,
) -> crate::cache::types::GameCacheEntry {
    crate::cache::types::GameCacheEntry {
        schema_version: crate::cache::types::CURRENT_SCHEMA_VERSION,
        app_id: summary.app_id,
        name: summary.name,
        steam_last_played: 0,
        cached_at: summary.cached_at,
        achievements: Vec::new(),
        stats: Vec::new(),
        progress: summary.progress,
        tier_breakdown: summary.tier_breakdown,
        genre: summary.genre,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::store::atomic_write;
    use crate::cache::types::{CachedProgress, GameSummaryCache, LAYER_SCHEMA_VERSION};

    fn make_summary_input(app_id: u32, last_played: Option<u32>) -> GameSummary {
        GameSummary {
            app_id,
            change_number: 0,
            last_played,
        }
    }

    fn make_summary(app_id: u32, cached_at: u64, change_number: u32) -> GameSummaryCache {
        GameSummaryCache {
            schema_version: LAYER_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            cached_change_number: change_number,
            cached_at,
            progress: CachedProgress {
                earned: 1,
                total: 10,
            },
            tier_breakdown: Vec::new(),
            genre: None,
        }
    }

    async fn write_summary(cache_root: &Path, app_id: u32, summary: &GameSummaryCache) {
        let path = cache_root.join(app_id.to_string()).join("summary.json");
        let bytes = serde_json::to_vec_pretty(summary).unwrap();
        atomic_write(&path, &bytes).await.unwrap();
    }

    #[tokio::test]
    async fn empty_games_list_produces_empty_result() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let result = classify_games_with_root(&[], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert!(result.dirty.is_empty());
        assert_eq!(result.schema_bumped, 0);
    }

    #[tokio::test]
    async fn no_cache_file_goes_dirty() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(1, None);

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![1]);
        assert_eq!(result.schema_bumped, 0);
        assert_eq!(result.invalidation_count, 1);
    }

    #[tokio::test]
    async fn cache_present_no_last_played_is_hit() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(2, None);

        let summary = make_summary(2, 999_999_999, 0);
        write_summary(cache_dir.path(), 2, &summary).await;

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert_eq!(result.hits.len(), 1, "should be a cache hit");
        assert!(result.dirty.is_empty());
        assert_eq!(result.hits[0].app_id, 2);
        assert_eq!(result.invalidation_count, 0);
    }

    #[tokio::test]
    async fn last_played_newer_than_cached_at_goes_dirty() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let cached_at: u64 = 500;
        let game = make_summary_input(4, Some(1000));

        let summary = make_summary(4, cached_at, 0);
        write_summary(cache_dir.path(), 4, &summary).await;

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![4]);
        assert_eq!(result.invalidation_count, 1);
    }

    #[tokio::test]
    async fn cache_present_last_played_older_than_cached_at_is_hit() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(5, Some(100));

        let summary = make_summary(5, 999_999_999, 0);
        write_summary(cache_dir.path(), 5, &summary).await;

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert_eq!(
            result.hits.len(),
            1,
            "game not played since last cache write must be a hit"
        );
        assert!(result.dirty.is_empty());
        assert_eq!(result.hits[0].app_id, 5);
        assert_eq!(result.invalidation_count, 0);
    }

    #[tokio::test]
    async fn schema_version_mismatch_goes_dirty_with_count() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(6, None);

        let subdir = cache_dir.path().join("6");
        std::fs::create_dir_all(&subdir).unwrap();
        let bad_cache = subdir.join("summary.json");
        let bad_json = r#"{"schema_version":99,"app_id":6,"name":"Game 6","cached_change_number":0,"cached_at":0,"progress":{"earned":0,"total":0},"tier_breakdown":[],"genre":null}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![6]);
        assert_eq!(result.schema_bumped, 1);
        assert_eq!(result.invalidation_count, 1);
    }

    #[tokio::test]
    async fn schema_version_zero_goes_dirty_and_increments_schema_bumped() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(7, None);

        let subdir = cache_dir.path().join("7");
        std::fs::create_dir_all(&subdir).unwrap();
        let bad_cache = subdir.join("summary.json");
        let bad_json = r#"{"schema_version":0,"app_id":7,"name":"Game 7","cached_change_number":0,"cached_at":0,"progress":{"earned":0,"total":0},"tier_breakdown":[],"genre":null}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty(), "version-0 cache must not be a hit");
        assert_eq!(result.dirty, vec![7], "version-0 cache must be dirty");
        assert_eq!(
            result.schema_bumped, 1,
            "schema_version=0 must increment schema_bumped like any other mismatch"
        );
        assert_eq!(result.invalidation_count, 1);
    }

    #[tokio::test]
    async fn change_number_diff_goes_dirty() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let mut game = make_summary_input(8, None);
        game.change_number = 42;

        let summary = make_summary(8, 999_999_999, 7);
        write_summary(cache_dir.path(), 8, &summary).await;

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(
            result.hits.is_empty(),
            "change_number mismatch must invalidate"
        );
        assert_eq!(result.dirty, vec![8]);
        assert_eq!(result.invalidation_count, 1);
        assert_eq!(
            result.schema_bumped, 0,
            "change_number diff is not a schema bump"
        );
    }
}
