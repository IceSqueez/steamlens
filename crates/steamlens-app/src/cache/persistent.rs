use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use steamlens_core::GameSummary;

use crate::cache::store::{CacheIoError, atomic_write};

const CURRENT_PROFILE_SCHEMA: u32 = 1;
const CURRENT_LIBRARY_SCHEMA: u32 = 1;

/// Persistent profile snapshot written after every successful Steam probe.
///
/// Used as fallback when a future boot finds Steam not running. The avatar PNG
/// is embedded directly so a single file restore is enough — keeps disk layout
/// trivial at the cost of a few extra KiB per profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedProfile {
    pub schema_version: u32,
    pub steam_id: u64,
    pub persona_name: String,
    pub account_name: String,
    pub avatar_png_bytes: Option<Vec<u8>>,
    /// Unix timestamp of when this snapshot was written.
    pub cached_at: u64,
}

/// Persistent library snapshot written after every successful library scan.
///
/// On Steam-not-running boots, restored from disk so the user still sees
/// their last-known game list (with cards rendering from per-game caches).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLibrary {
    pub schema_version: u32,
    pub games: Vec<GameSummary>,
    pub cached_at: u64,
}

fn profile_path() -> PathBuf {
    crate::settings::steamlens_root()
        .join("cache")
        .join("profile.json")
}

fn library_path() -> PathBuf {
    crate::settings::steamlens_root()
        .join("cache")
        .join("library.json")
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Atomically write the profile snapshot to its canonical path.
pub async fn write_profile_cache(profile: &CachedProfile) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(profile).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(&profile_path(), &bytes).await
}

/// Read the profile snapshot from disk. Returns `None` when the file is
/// missing, unparsable, or has a stale `schema_version`.
pub async fn load_profile_cache() -> Option<CachedProfile> {
    load_profile_cache_from_path(&profile_path()).await
}

pub(crate) async fn load_profile_cache_from_path(path: &std::path::Path) -> Option<CachedProfile> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: CachedProfile = serde_json::from_slice(&bytes)
        .map_err(|e| {
            eprintln!(
                "[steamlens] profile cache: parse error at {}: {e}",
                path.display()
            );
        })
        .ok()?;
    if entry.schema_version != CURRENT_PROFILE_SCHEMA {
        eprintln!(
            "[steamlens] profile cache: schema {} != expected {}; treating as miss",
            entry.schema_version, CURRENT_PROFILE_SCHEMA
        );
        return None;
    }
    Some(entry)
}

/// Atomically write the library snapshot to its canonical path.
pub async fn write_library_cache(library: &CachedLibrary) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(library).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(&library_path(), &bytes).await
}

/// Read the library snapshot from disk. Returns `None` when the file is
/// missing, unparsable, or has a stale `schema_version`.
pub async fn load_library_cache() -> Option<CachedLibrary> {
    load_library_cache_from_path(&library_path()).await
}

pub(crate) async fn load_library_cache_from_path(path: &std::path::Path) -> Option<CachedLibrary> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: CachedLibrary = serde_json::from_slice(&bytes)
        .map_err(|e| {
            eprintln!(
                "[steamlens] library cache: parse error at {}: {e}",
                path.display()
            );
        })
        .ok()?;
    if entry.schema_version != CURRENT_LIBRARY_SCHEMA {
        eprintln!(
            "[steamlens] library cache: schema {} != expected {}; treating as miss",
            entry.schema_version, CURRENT_LIBRARY_SCHEMA
        );
        return None;
    }
    Some(entry)
}

/// Build a `CachedProfile` from current live data ready for `write_profile_cache`.
pub fn make_cached_profile(
    steam_id: u64,
    persona_name: String,
    account_name: String,
    avatar_png_bytes: Option<Vec<u8>>,
) -> CachedProfile {
    CachedProfile {
        schema_version: CURRENT_PROFILE_SCHEMA,
        steam_id,
        persona_name,
        account_name,
        avatar_png_bytes,
        cached_at: now_epoch(),
    }
}

/// Build a `CachedLibrary` from a freshly scanned game list ready for `write_library_cache`.
pub fn make_cached_library(games: Vec<GameSummary>) -> CachedLibrary {
    CachedLibrary {
        schema_version: CURRENT_LIBRARY_SCHEMA,
        games,
        cached_at: now_epoch(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_persistent_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn make_profile() -> CachedProfile {
        CachedProfile {
            schema_version: CURRENT_PROFILE_SCHEMA,
            steam_id: 76561198000000042,
            persona_name: "TestUser".to_owned(),
            account_name: "test_login".to_owned(),
            avatar_png_bytes: Some(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            cached_at: 1_777_926_953,
        }
    }

    fn make_library() -> CachedLibrary {
        CachedLibrary {
            schema_version: CURRENT_LIBRARY_SCHEMA,
            games: vec![GameSummary {
                app_id: 105600,
                name: "Terraria".to_owned(),
                last_played: Some(1_777_926_953),
                achievement_count: 88,
                last_updated: 1_770_000_000,
                manifest_path: PathBuf::from("/tmp/appmanifest_105600.acf"),
            }],
            cached_at: 1_777_926_953,
        }
    }

    #[tokio::test]
    async fn profile_cache_round_trip_via_explicit_path() {
        let dir = tempdir();
        let path = dir.join("profile.json");
        let original = make_profile();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&path, &bytes).await.unwrap();

        let restored = load_profile_cache_from_path(&path)
            .await
            .expect("must load");
        assert_eq!(restored.steam_id, original.steam_id);
        assert_eq!(restored.persona_name, original.persona_name);
        assert_eq!(restored.account_name, original.account_name);
        assert_eq!(restored.avatar_png_bytes, original.avatar_png_bytes);
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn profile_cache_missing_file_returns_none() {
        let dir = tempdir();
        let result = load_profile_cache_from_path(&dir.join("does_not_exist.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_corrupted_json_returns_none() {
        let dir = tempdir();
        let path = dir.join("corrupted.json");
        std::fs::write(&path, b"this isn't json {{{").unwrap();
        let result = load_profile_cache_from_path(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_schema_mismatch_returns_none() {
        let dir = tempdir();
        let path = dir.join("schema.json");
        let bad = r#"{"schema_version":99,"steam_id":1,"persona_name":"X","account_name":"x","avatar_png_bytes":null,"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_profile_cache_from_path(&path).await;
        assert!(result.is_none(), "stale schema must be treated as miss");
    }

    #[tokio::test]
    async fn library_cache_round_trip_via_explicit_path() {
        let dir = tempdir();
        let path = dir.join("library.json");
        let original = make_library();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&path, &bytes).await.unwrap();

        let restored = load_library_cache_from_path(&path)
            .await
            .expect("must load");
        assert_eq!(restored.games.len(), 1);
        assert_eq!(restored.games[0].app_id, 105600);
        assert_eq!(restored.games[0].name, "Terraria");
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn library_cache_missing_file_returns_none() {
        let dir = tempdir();
        let result = load_library_cache_from_path(&dir.join("nope.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_corrupted_json_returns_none() {
        let dir = tempdir();
        let path = dir.join("corrupted.json");
        std::fs::write(&path, b"][}}}").unwrap();
        let result = load_library_cache_from_path(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_schema_mismatch_returns_none() {
        let dir = tempdir();
        let path = dir.join("library.json");
        let bad = r#"{"schema_version":99,"games":[],"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_library_cache_from_path(&path).await;
        assert!(result.is_none());
    }

    #[test]
    fn make_cached_profile_sets_schema_and_timestamp() {
        let p = make_cached_profile(1, "u".into(), "l".into(), None);
        assert_eq!(p.schema_version, CURRENT_PROFILE_SCHEMA);
        assert!(p.cached_at > 0, "cached_at must be set to a real epoch");
    }

    #[test]
    fn make_cached_library_sets_schema_and_timestamp() {
        let l = make_cached_library(Vec::new());
        assert_eq!(l.schema_version, CURRENT_LIBRARY_SCHEMA);
        assert!(l.cached_at > 0);
    }
}
