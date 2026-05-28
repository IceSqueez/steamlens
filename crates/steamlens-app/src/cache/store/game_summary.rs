use std::path::Path;

use crate::cache::types::{GameSummaryCache, SUMMARY_SCHEMA_VERSION};

use super::primitives::{CacheIoError, atomic_write};

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
