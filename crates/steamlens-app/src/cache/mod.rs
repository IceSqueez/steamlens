pub mod invalidate;
pub mod persistent;
pub mod store;
pub mod types;

pub use invalidate::{ClassifyResult, classify_games};
pub use persistent::{
    CachedLibrary, CachedProfile, load_library_cache, load_profile_cache, make_cached_library,
    make_cached_profile, write_library_cache, write_profile_cache,
};
pub use store::{game_cache_path, write_game_cache};
pub use types::{CURRENT_SCHEMA_VERSION, CacheHit, GameCacheEntry};
