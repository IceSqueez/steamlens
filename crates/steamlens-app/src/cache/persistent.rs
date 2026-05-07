use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cache::cached::Cached;
use crate::cache::store::CacheIoError;

const CURRENT_PROFILE_SCHEMA: u32 = 2;
const CURRENT_LIBRARY_SCHEMA: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedProfile {
    pub schema_version: u32,
    pub steam_id: u64,
    pub persona_name: String,
    pub account_name: String,
    pub avatar_png_bytes: Option<Vec<u8>>,
    pub steam_root: Option<PathBuf>,
    pub cached_at: u64,
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

impl Cached for CachedProfile {
    const NAME: &'static str = "profile";
    const CURRENT_SCHEMA: u32 = CURRENT_PROFILE_SCHEMA;
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
    fn path() -> PathBuf {
        profile_path()
    }
}

impl Cached for CachedLibrary {
    const NAME: &'static str = "library";
    const CURRENT_SCHEMA: u32 = CURRENT_LIBRARY_SCHEMA;
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
    fn path() -> PathBuf {
        library_path()
    }
}

pub async fn write_profile_cache(profile: &CachedProfile) -> Result<(), CacheIoError> {
    crate::cache::cached::write(profile).await
}

pub async fn load_profile_cache() -> Option<CachedProfile> {
    crate::cache::cached::load::<CachedProfile>().await
}

pub async fn write_library_cache(library: &CachedLibrary) -> Result<(), CacheIoError> {
    crate::cache::cached::write(library).await
}

pub async fn load_library_cache() -> Option<CachedLibrary> {
    crate::cache::cached::load::<CachedLibrary>().await
}

pub fn make_cached_profile(
    steam_id: u64,
    persona_name: String,
    account_name: String,
    avatar_png_bytes: Option<Vec<u8>>,
    steam_root: Option<PathBuf>,
) -> CachedProfile {
    CachedProfile {
        schema_version: CURRENT_PROFILE_SCHEMA,
        steam_id,
        persona_name,
        account_name,
        avatar_png_bytes,
        steam_root,
        cached_at: now_epoch(),
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
    use crate::cache::cached::load_from_path;
    use crate::cache::store::atomic_write;

    fn make_profile() -> CachedProfile {
        CachedProfile {
            schema_version: CURRENT_PROFILE_SCHEMA,
            steam_id: 76561198000000042,
            persona_name: "TestUser".to_owned(),
            account_name: "test_login".to_owned(),
            avatar_png_bytes: Some(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            steam_root: Some(PathBuf::from("/tmp/synthetic_steam_root")),
            cached_at: 1_777_926_953,
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

        let restored = load_from_path::<CachedProfile>(&path)
            .await
            .expect("must load");
        assert_eq!(restored.steam_id, original.steam_id);
        assert_eq!(restored.persona_name, original.persona_name);
        assert_eq!(restored.account_name, original.account_name);
        assert_eq!(restored.avatar_png_bytes, original.avatar_png_bytes);
        assert_eq!(restored.steam_root, original.steam_root);
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn profile_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let result = load_from_path::<CachedProfile>(&dir.path().join("does_not_exist.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"this isn't json {{{").unwrap();
        let result = load_from_path::<CachedProfile>(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn profile_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("schema.json");
        let bad = r#"{"schema_version":1,"steam_id":1,"persona_name":"X","account_name":"x","avatar_png_bytes":null,"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_from_path::<CachedProfile>(&path).await;
        assert!(result.is_none(), "stale schema must be treated as miss");
    }

    #[tokio::test]
    async fn library_cache_round_trip_via_explicit_path() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("library.json");
        let original = make_library();
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        atomic_write(&path, &bytes).await.unwrap();

        let restored = load_from_path::<CachedLibrary>(&path)
            .await
            .expect("must load");
        assert_eq!(restored.games.len(), 1);
        assert_eq!(restored.games[0].app_id, 105600);
        assert_eq!(restored.games[0].name, "Terraria");
        assert_eq!(restored.games[0].achievement_count, 88);
        assert_eq!(restored.cached_at, original.cached_at);
    }

    #[tokio::test]
    async fn library_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let result = load_from_path::<CachedLibrary>(&dir.path().join("nope.json")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"][}}}").unwrap();
        let result = load_from_path::<CachedLibrary>(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn library_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("library.json");
        let bad = r#"{"schema_version":2,"games":[],"cached_at":0}"#;
        std::fs::write(&path, bad).unwrap();
        let result = load_from_path::<CachedLibrary>(&path).await;
        assert!(result.is_none());
    }

    #[test]
    fn make_cached_profile_sets_schema_and_timestamp() {
        let p = make_cached_profile(1, "u".into(), "l".into(), None, None);
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
