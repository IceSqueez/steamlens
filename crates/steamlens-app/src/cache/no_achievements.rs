//! Persistent cache of app_ids that scanned as having no achievements.
//!
//! Records the package's `change_number` at scan time so the entry can be
//! invalidated when Steam pushes any update for the package (which advances
//! the change_number). The cache lets cold-start subsequent runs skip
//! scanning the ~hundreds of owned non-achievement games (DLCs that slipped
//! the type filter, free-weekend leftovers, games whose stats schema Steam
//! never configured, etc.).
//!
//! Storage: `<XDG_CACHE>/steamlens/cache/no_achievements.json`.
//! Atomic write via `.tmp` + rename. Schema-versioned; old caches are
//! discarded on version mismatch (no shims, per CLAUDE.md).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cache::store::{CacheIoError, atomic_write};

pub const CURRENT_NO_ACHIEVEMENTS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NoAchievementsCache {
    pub schema_version: u32,
    /// app_id → package change_number observed when "no achievements" was recorded.
    pub entries: HashMap<u32, u32>,
}

impl NoAchievementsCache {
    pub fn new() -> Self {
        Self {
            schema_version: CURRENT_NO_ACHIEVEMENTS_SCHEMA,
            entries: HashMap::new(),
        }
    }

    /// Returns `true` when `app_id` is recorded and the recorded
    /// `change_number` exactly matches `current`. A mismatch (or absence)
    /// returns `false` — the caller must scan the app again.
    pub fn is_known_empty(&self, app_id: u32, current_change: u32) -> bool {
        match self.entries.get(&app_id) {
            Some(&recorded) => recorded == current_change,
            None => false,
        }
    }

    pub fn insert(&mut self, app_id: u32, change_number: u32) {
        self.entries.insert(app_id, change_number);
    }
}

pub fn cache_path() -> PathBuf {
    crate::settings::steamlens_root()
        .join("cache")
        .join("no_achievements.json")
}

pub async fn load() -> NoAchievementsCache {
    let path = cache_path();
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return NoAchievementsCache::new();
    };
    let parsed: Result<NoAchievementsCache, _> = serde_json::from_slice(&bytes);
    match parsed {
        Ok(c) if c.schema_version == CURRENT_NO_ACHIEVEMENTS_SCHEMA => c,
        Ok(_) => {
            eprintln!(
                "[steamlens] no_achievements cache: schema mismatch (expected {}); discarding",
                CURRENT_NO_ACHIEVEMENTS_SCHEMA,
            );
            NoAchievementsCache::new()
        }
        Err(e) => {
            eprintln!("[steamlens] no_achievements cache: parse failed: {e}; discarding");
            NoAchievementsCache::new()
        }
    }
}

pub async fn write(cache: &NoAchievementsCache) -> Result<(), CacheIoError> {
    let path = cache_path();
    let bytes = serde_json::to_vec_pretty(cache)
        .map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(&path, &bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_known_empty_matches_change_number() {
        let mut cache = NoAchievementsCache::new();
        cache.insert(12345, 100);
        assert!(cache.is_known_empty(12345, 100));
        assert!(!cache.is_known_empty(12345, 101)); // change_number bumped
        assert!(!cache.is_known_empty(99999, 100)); // not in cache
    }

    #[test]
    fn insert_overwrites_change_number() {
        let mut cache = NoAchievementsCache::new();
        cache.insert(12345, 100);
        cache.insert(12345, 200);
        assert!(cache.is_known_empty(12345, 200));
        assert!(!cache.is_known_empty(12345, 100));
    }

    #[test]
    fn empty_cache_treats_all_as_unknown() {
        let cache = NoAchievementsCache::new();
        assert!(!cache.is_known_empty(1, 0));
        assert!(!cache.is_known_empty(0, 0));
    }

    #[test]
    fn schema_round_trip() {
        let mut cache = NoAchievementsCache::new();
        cache.insert(12345, 7777);
        cache.insert(67890, 8888);

        let bytes = serde_json::to_vec(&cache).unwrap();
        let restored: NoAchievementsCache = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.schema_version, CURRENT_NO_ACHIEVEMENTS_SCHEMA);
        assert!(restored.is_known_empty(12345, 7777));
        assert!(restored.is_known_empty(67890, 8888));
    }
}
