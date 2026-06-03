pub mod commands;
pub mod icons;
pub mod invalidate;
pub mod migration;
pub mod no_achievements;
pub mod persistent;
pub mod store;
pub mod types;

pub use invalidate::{ClassifyResult, classify_games};
pub use no_achievements::{
    NoAchievementsCache, load_blocking as load_no_achievements_cache_blocking,
    write as write_no_achievements_cache,
};
pub use persistent::{
    CachedLibrary, CachedLibraryEntry, CachedProfile, load_library_cache, load_profile_cache,
    make_cached_library, make_cached_profile, write_library_cache, write_profile_cache,
};
pub use store::write_game_cache;
pub use types::{CURRENT_SCHEMA_VERSION, CacheHit, GameCacheEntry};

#[derive(Debug, Clone)]
pub enum CacheEvent {
    ProfileLoaded(Option<CachedProfile>),
    LibraryLoaded(Option<CachedLibrary>),
    PersistentWritten(&'static str, Result<(), String>),
    Classified(ClassifyResult),
    GameWritten {
        app_id: u32,
        result: Result<(), String>,
    },
    NoAchWritten(Result<(), String>),
    GameInvalidated {
        app_id: u32,
        name: String,
        result: Result<(), String>,
    },
    OfflineLoaded {
        app_id: u32,
        entry: Option<Box<GameCacheEntry>>,
    },
}
