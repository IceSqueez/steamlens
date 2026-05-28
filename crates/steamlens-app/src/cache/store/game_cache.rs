use std::path::{Path, PathBuf};

use crate::cache::types::{CURRENT_SCHEMA_VERSION, GameCacheEntry};

use super::primitives::{CacheIoError, atomic_write, cache_write_lock};

fn game_cache_path(app_id: u32) -> PathBuf {
    crate::paths::cache_dir()
        .join("games")
        .join(format!("{app_id}.json"))
}

pub async fn load_game_cache(app_id: u32) -> Option<GameCacheEntry> {
    load_game_cache_from_path(&game_cache_path(app_id)).await
}

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
    write_game_cache_at(&game_cache_path(entry.app_id), entry).await
}

pub(super) async fn write_game_cache_at(
    path: &Path,
    entry: &GameCacheEntry,
) -> Result<(), CacheIoError> {
    let lock = cache_write_lock(entry.app_id);
    let _guard = lock.lock().await;
    let mut merged = entry.clone();
    if let Some(old) = load_game_cache_from_path(path).await {
        merge_preserved_fields(&mut merged, &old);
    }
    let bytes =
        serde_json::to_vec_pretty(&merged).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(path, &bytes).await
}

pub(crate) fn merge_preserved_fields(new: &mut GameCacheEntry, old: &GameCacheEntry) {
    use std::collections::HashMap;
    let old_by_id: HashMap<&str, &crate::cache::types::CachedAchievement> = old
        .achievements
        .iter()
        .map(|a| (a.api_name.as_str(), a))
        .collect();
    for ach in &mut new.achievements {
        let Some(prev) = old_by_id.get(ach.api_name.as_str()) else {
            continue;
        };
        if ach.display_name.is_empty() && !prev.display_name.is_empty() {
            ach.display_name = prev.display_name.clone();
        }
        if ach.description.is_empty() && !prev.description.is_empty() {
            ach.description = prev.description.clone();
        }
        if !ach.hidden && prev.hidden {
            ach.hidden = true;
        }
    }
    if new.genre.is_none() && old.genre.is_some() {
        new.genre = old.genre.clone();
    }
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
