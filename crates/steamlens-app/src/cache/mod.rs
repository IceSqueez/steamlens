pub mod invalidate;
pub mod no_achievements;
pub mod persistent;
pub mod store;
pub mod types;

pub use invalidate::{ClassifyResult, classify_games};
pub use no_achievements::{NoAchievementsCache, load as load_no_achievements_cache, write as write_no_achievements_cache};
pub use persistent::{
    CachedLibrary, CachedProfile, load_library_cache, load_profile_cache, make_cached_library,
    make_cached_profile, write_library_cache, write_profile_cache,
};
pub use store::{game_cache_path, write_game_cache};
pub use types::{CURRENT_SCHEMA_VERSION, CacheHit, GameCacheEntry};
