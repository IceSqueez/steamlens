use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cache::store::CacheIoError;

const CURRENT_PROFILE_SCHEMA: u32 = 5;
const CURRENT_LIBRARY_SCHEMA: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedProfile {
    pub schema_version: u32,
    pub steam_id: u64,
    pub nickname: String,
    pub avatar_png_bytes: Option<Vec<u8>>,
    pub steam_root: Option<PathBuf>,
    pub cached_at: u64,
    pub steam_level: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLibraryEntry {
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
    pub name: String,
    pub achievement_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLibrary {
    pub schema_version: u32,
    pub games: Vec<CachedLibraryEntry>,
    pub cached_at: u64,
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn write_profile_cache(
    steamid3: u32,
    profile: &CachedProfile,
) -> Result<(), CacheIoError> {
    let path = crate::paths::user_profile_path(steamid3);
    let bytes =
        serde_json::to_vec_pretty(profile).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    crate::cache::store::atomic_write(&path, &bytes).await
}

pub async fn load_profile_cache(steamid3: u32) -> Option<CachedProfile> {
    let path = crate::paths::user_profile_path(steamid3);
    load_profile_from_path(&path).await
}

pub(crate) async fn load_profile_from_path(path: &PathBuf) -> Option<CachedProfile> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: CachedProfile = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!("profile cache: parse error at {}: {e}", path.display());
        })
        .ok()?;
    if entry.schema_version != CURRENT_PROFILE_SCHEMA {
        tracing::warn!(
            "profile cache: schema {} != expected {}; treating as miss",
            entry.schema_version,
            CURRENT_PROFILE_SCHEMA
        );
        return None;
    }
    Some(entry)
}

pub async fn write_library_cache(
    steamid3: u32,
    library: &CachedLibrary,
) -> Result<(), CacheIoError> {
    let path = crate::paths::user_library_path(steamid3);
    let bytes =
        serde_json::to_vec_pretty(library).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    crate::cache::store::atomic_write(&path, &bytes).await
}

pub async fn load_library_cache(steamid3: u32) -> Option<CachedLibrary> {
    let path = crate::paths::user_library_path(steamid3);
    load_library_from_path(&path).await
}

pub(crate) async fn load_library_from_path(path: &PathBuf) -> Option<CachedLibrary> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: CachedLibrary = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::warn!("library cache: parse error at {}: {e}", path.display());
        })
        .ok()?;
    if entry.schema_version != CURRENT_LIBRARY_SCHEMA {
        tracing::warn!(
            "library cache: schema {} != expected {}; treating as miss",
            entry.schema_version,
            CURRENT_LIBRARY_SCHEMA
        );
        return None;
    }
    Some(entry)
}

pub fn make_cached_profile(
    steam_id: u64,
    nickname: String,
    avatar_png_bytes: Option<Vec<u8>>,
    steam_root: Option<PathBuf>,
    steam_level: Option<u32>,
) -> CachedProfile {
    CachedProfile {
        schema_version: CURRENT_PROFILE_SCHEMA,
        steam_id,
        nickname,
        avatar_png_bytes,
        steam_root,
        cached_at: now_epoch(),
        steam_level,
    }
}

pub fn make_cached_library(games: Vec<CachedLibraryEntry>) -> CachedLibrary {
    CachedLibrary {
        schema_version: CURRENT_LIBRARY_SCHEMA,
        games,
        cached_at: now_epoch(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::store::atomic_write;

    const TEST_STEAMID3: u32 = 123456789;

    fn make_profile() -> CachedProfile {
        CachedProfile {
            schema_version: CURRENT_PROFILE_SCHEMA,
            steam_id: 76561198000000042,
            nickname: "TestUser".to_owned(),
            avatar_png_bytes: Some(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            steam_root: Some(PathBuf::from("/tmp/synthetic_steam_root")),
            cached_at: 1_777_926_953,
            steam_level: None,
        }
    }

    fn make_library() -> CachedLibrary {
        CachedLibrary {
            schema_version: CURRENT_LIBRARY_SCHEMA,
            games: vec![CachedLibraryEntry {
                app_id: 105600,
                change_number: 0,
                last_played: Some(1_777_926_953),
                name: "Terraria".to_owned(),
                achievement_count: 88,
            }],
            cached_at: 1_777_926_953,
        }
    }

    #[tokio::test]
    async fn profile_cache_round_trip_via_explicit_path() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("profile.json");
        let original = make_profile();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&path, &bytes).await.unwrap();

        let restored = load_profile_from_path(&path).await.expect("must load");
        assert_eq!(restored.steam_id, original.steam_id);
        assert_eq!(restored.nickname, original.nickname);
        assert_eq!(restored.avatar_png_bytes, original.avatar_png_bytes);
        assert_eq!(restored.steam_root, original.steam_root);
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn profile_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let result = load_profile_from_path(&dir.path().join("does_not_exist.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"this isn't json {{{").unwrap();
        let result = load_profile_from_path(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("schema.json");
        let bad = r#"{"schema_version":1,"steam_id":1,"nickname":"X","avatar_png_bytes":null,"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_profile_from_path(&path).await;
        assert!(result.is_none(), "stale schema must be treated as miss");
    }

    #[tokio::test]
    async fn library_cache_round_trip_via_explicit_path() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("library.json");
        let original = make_library();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&path, &bytes).await.unwrap();

        let restored = load_library_from_path(&path).await.expect("must load");
        assert_eq!(restored.games.len(), 1);
        assert_eq!(restored.games[0].app_id, 105600);
        assert_eq!(restored.games[0].name, "Terraria");
        assert_eq!(restored.games[0].achievement_count, 88);
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn library_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let result = load_library_from_path(&dir.path().join("nope.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"][}}}").unwrap();
        let result = load_library_from_path(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("library.json");
        let bad = r#"{"schema_version":2,"games":[],"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_library_from_path(&path).await;
        assert!(result.is_none());
    }

    #[test]
    fn make_cached_profile_sets_schema_and_timestamp() {
        let p = make_cached_profile(1, "u".into(), None, None, None);
        assert_eq!(p.schema_version, CURRENT_PROFILE_SCHEMA);
        assert!(p.cached_at > 0, "cached_at must be set to a real epoch");
    }

    #[test]
    fn make_cached_library_sets_schema_and_timestamp() {
        let l = make_cached_library(Vec::new());
        assert_eq!(l.schema_version, CURRENT_LIBRARY_SCHEMA);
        assert!(l.cached_at > 0);
    }

    #[tokio::test]
    async fn write_and_load_profile_cache_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let user_path = dir
            .path()
            .join("users")
            .join(TEST_STEAMID3.to_string())
            .join("profile.json");
        std::fs::create_dir_all(user_path.parent().unwrap()).unwrap();

        let original = make_profile();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&user_path, &bytes).await.unwrap();

        let restored = load_profile_from_path(&user_path).await.expect("must load");
        assert_eq!(restored.steam_id, original.steam_id);
        assert_eq!(restored.schema_version, CURRENT_PROFILE_SCHEMA);
    }

    #[tokio::test]
    async fn write_and_load_library_cache_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let lib_path = dir
            .path()
            .join("users")
            .join(TEST_STEAMID3.to_string())
            .join("library.json");
        std::fs::create_dir_all(lib_path.parent().unwrap()).unwrap();

        let original = make_library();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&lib_path, &bytes).await.unwrap();

        let restored = load_library_from_path(&lib_path).await.expect("must load");
        assert_eq!(restored.games.len(), 1);
        assert_eq!(restored.schema_version, CURRENT_LIBRARY_SCHEMA);
    }
}
