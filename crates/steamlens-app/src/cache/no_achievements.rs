use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cache::cached::Cached;
use crate::cache::store::CacheIoError;

pub const CURRENT_NO_ACHIEVEMENTS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoAchievementsCache {
    pub schema_version: u32,
    pub entries: HashMap<u32, u32>,
}

impl NoAchievementsCache {
    pub fn new() -> Self {
        Self {
            schema_version: CURRENT_NO_ACHIEVEMENTS_SCHEMA,
            entries: HashMap::new(),
        }
    }

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

impl Cached for NoAchievementsCache {
    fn path() -> PathBuf {
        cache_path()
    }
}

pub fn cache_path() -> PathBuf {
    crate::paths::no_achievements_path()
}

pub fn load_blocking() -> NoAchievementsCache {
    let path = cache_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return NoAchievementsCache::new();
    };
    let parsed: NoAchievementsCache = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "no_achievements cache: parse error at {}: {e}",
                path.display()
            );
            return NoAchievementsCache::new();
        }
    };
    if parsed.schema_version != CURRENT_NO_ACHIEVEMENTS_SCHEMA {
        tracing::warn!(
            "no_achievements cache: schema {} != expected {}; treating as miss",
            parsed.schema_version,
            CURRENT_NO_ACHIEVEMENTS_SCHEMA
        );
        return NoAchievementsCache::new();
    }
    parsed
}

pub async fn write(cache: &NoAchievementsCache) -> Result<(), CacheIoError> {
    crate::cache::cached::write(cache).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_known_empty_matches_change_number() {
        let mut cache = NoAchievementsCache::new();
        cache.insert(12345, 100);
        assert!(cache.is_known_empty(12345, 100));
        assert!(!cache.is_known_empty(12345, 101));
        assert!(!cache.is_known_empty(99999, 100));
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
