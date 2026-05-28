use crate::cache::types::{GameAchievementsCache, SUMMARY_SCHEMA_VERSION};

use super::primitives::{CacheIoError, atomic_write};

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
