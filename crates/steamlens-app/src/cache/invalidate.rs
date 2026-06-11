use std::collections::HashSet;
use std::path::Path;

use steamlens_core::GameSummary;

use crate::cache::CacheHit;
use crate::cache::store::load_game_cache_from_path;

const LAST_PLAYED_RACE_GRACE_SECS: u64 = 30;

#[derive(Debug, Clone, Default)]
pub struct ClassifyResult {
    pub hits: Vec<CacheHit>,
    pub dirty: Vec<u32>,
    pub schema_bumped: u32,
    pub invalidation_count: u32,
}

pub async fn classify_games(
    game_summaries: &[GameSummary],
    _steam_root: &Path,
    account_id: u32,
) -> ClassifyResult {
    let user_games_root = crate::paths::user_dir(account_id).join("games");
    classify_games_with_root(game_summaries, &user_games_root).await
}

async fn scan_cached_app_ids(cache_root: &Path) -> HashSet<u32> {
    let mut set = HashSet::new();
    let Ok(mut entries) = tokio::fs::read_dir(cache_root).await else {
        return set;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(app_id) = name.parse::<u32>() else {
            continue;
        };
        set.insert(app_id);
    }
    set
}

pub(crate) async fn classify_games_with_root(
    game_summaries: &[GameSummary],
    cache_root: &Path,
) -> ClassifyResult {
    let mut result = ClassifyResult::default();
    let mut no_cache_count: u32 = 0;

    let cached_app_ids = scan_cached_app_ids(cache_root).await;

    let mut load_set: tokio::task::JoinSet<(usize, Option<crate::cache::types::GameCacheEntry>)> =
        tokio::task::JoinSet::new();
    for (idx, game) in game_summaries.iter().enumerate() {
        if !cached_app_ids.contains(&game.app_id) {
            continue;
        }
        let path = cache_root.join(game.app_id.to_string()).join("cache.json");
        load_set.spawn(async move { (idx, load_game_cache_from_path(&path).await) });
    }

    let mut loaded: std::collections::HashMap<usize, Option<crate::cache::types::GameCacheEntry>> =
        std::collections::HashMap::with_capacity(load_set.len());
    while let Some(res) = load_set.join_next().await {
        if let Ok((idx, entry)) = res {
            loaded.insert(idx, entry);
        }
    }

    for (idx, game) in game_summaries.iter().enumerate() {
        let app_id = game.app_id;

        if !cached_app_ids.contains(&app_id) {
            no_cache_count += 1;
            result.dirty.push(app_id);
            continue;
        }

        let Some(Some(entry)) = loaded.remove(&idx) else {
            tracing::info!(
                "invalidate app_id={app_id} reason={:?}",
                InvalidationReason::SchemaVersion
            );
            result.schema_bumped += 1;
            result.dirty.push(app_id);
            continue;
        };

        if entry.cached_change_number != game.change_number {
            tracing::info!(
                "invalidate app_id={app_id} reason={:?}",
                InvalidationReason::ChangeNumber
            );
            result.dirty.push(app_id);
            result.invalidation_count += 1;
            continue;
        }

        if let Some(last_played) = game.last_played
            && (last_played as u64) > entry.cached_at + LAST_PLAYED_RACE_GRACE_SECS
        {
            tracing::warn!(
                "invalidate app_id={app_id} reason={:?} last_played={last_played} cached_at={} grace={LAST_PLAYED_RACE_GRACE_SECS}s",
                InvalidationReason::LastPlayed,
                entry.cached_at
            );
            result.dirty.push(app_id);
            result.invalidation_count += 1;
            continue;
        }

        result.hits.push(CacheHit { app_id, entry });
    }

    if no_cache_count > 0 {
        tracing::info!("invalidate batch: {no_cache_count} games with reason=NoCache");
    }

    tracing::info!(
        "cache classify: {} hits, {} dirty, {} schema-bumped",
        result.hits.len(),
        result.dirty.len(),
        result.schema_bumped
    );

    result
}

#[derive(Debug)]
enum InvalidationReason {
    SchemaVersion,
    ChangeNumber,
    LastPlayed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::store::atomic_write;
    use crate::cache::types::{CURRENT_SCHEMA_VERSION, CachedProgress, GameCacheEntry};

    fn make_summary_input(app_id: u32, last_played: Option<u32>) -> GameSummary {
        GameSummary {
            app_id,
            change_number: 0,
            last_played,
        }
    }

    fn make_cache_entry(app_id: u32, cached_at: u64, change_number: u32) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            cached_change_number: change_number,
            steam_last_played: 0,
            cached_at,
            achievements: Vec::new(),
            stats: Vec::new(),
            progress: CachedProgress {
                earned: 1,
                total: 10,
            },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        }
    }

    async fn write_cache_entry(cache_root: &Path, app_id: u32, entry: &GameCacheEntry) {
        let path = cache_root.join(app_id.to_string()).join("cache.json");
        let bytes = serde_json::to_vec_pretty(entry).unwrap();
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
        assert_eq!(
            result.invalidation_count, 0,
            "no_cache is not user-visible invalidation"
        );
    }

    #[tokio::test]
    async fn cache_present_no_last_played_is_hit() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(2, None);

        let entry = make_cache_entry(2, 999_999_999, 0);
        write_cache_entry(cache_dir.path(), 2, &entry).await;

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

        let entry = make_cache_entry(4, cached_at, 0);
        write_cache_entry(cache_dir.path(), 4, &entry).await;

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![4]);
        assert_eq!(result.invalidation_count, 1);
    }

    #[tokio::test]
    async fn cache_present_last_played_older_than_cached_at_is_hit() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(5, Some(100));

        let entry = make_cache_entry(5, 999_999_999, 0);
        write_cache_entry(cache_dir.path(), 5, &entry).await;

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
        let bad_cache = subdir.join("cache.json");
        let bad_json = r#"{"schema_version":99,"app_id":6,"name":"Game 6","cached_change_number":0,"cached_at":0,"progress":{"earned":0,"total":0},"tier_breakdown":[],"genre":null}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty());
        assert_eq!(result.dirty, vec![6]);
        assert_eq!(result.schema_bumped, 1);
        assert_eq!(
            result.invalidation_count, 0,
            "schema bump is surfaced via banner, not invalidation_count"
        );
    }

    #[tokio::test]
    async fn schema_version_zero_goes_dirty_and_increments_schema_bumped() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let game = make_summary_input(7, None);

        let subdir = cache_dir.path().join("7");
        std::fs::create_dir_all(&subdir).unwrap();
        let bad_cache = subdir.join("cache.json");
        let bad_json = r#"{"schema_version":0,"app_id":7,"name":"Game 7","cached_change_number":0,"cached_at":0,"progress":{"earned":0,"total":0},"tier_breakdown":[],"genre":null}"#;
        std::fs::write(&bad_cache, bad_json).unwrap();

        let result = classify_games_with_root(&[game], cache_dir.path()).await;
        assert!(result.hits.is_empty(), "version-0 cache must not be a hit");
        assert_eq!(result.dirty, vec![7], "version-0 cache must be dirty");
        assert_eq!(
            result.schema_bumped, 1,
            "schema_version=0 must increment schema_bumped like any other mismatch"
        );
        assert_eq!(
            result.invalidation_count, 0,
            "schema bump is surfaced via banner, not invalidation_count"
        );
    }

    #[tokio::test]
    async fn change_number_diff_goes_dirty() {
        let cache_dir = tempfile::TempDir::new().expect("tempdir");
        let mut game = make_summary_input(8, None);
        game.change_number = 42;

        let entry = make_cache_entry(8, 999_999_999, 7);
        write_cache_entry(cache_dir.path(), 8, &entry).await;

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
