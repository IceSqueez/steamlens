use std::path::PathBuf;

use iced::Task;

use crate::cache::types::GameSummaryCache;
use crate::cache::{self, CacheEvent, CachedLibrary, GameCacheEntry, NoAchievementsCache};
use steamlens_core::GameSummary;

pub fn load_library_cache(steamid3: u32) -> Task<crate::Message> {
    Task::perform(
        async move { cache::load_library_cache(steamid3).await },
        |c| crate::Message::Cache(CacheEvent::LibraryLoaded(c)),
    )
}

pub fn load_profile_cache(steamid3: u32) -> Task<crate::Message> {
    Task::perform(
        async move { cache::load_profile_cache(steamid3).await },
        |c| crate::Message::Cache(CacheEvent::ProfileLoaded(c)),
    )
}

pub fn write_profile_cache(steamid3: u32, cached: cache::CachedProfile) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_profile_cache(steamid3, &cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::Cache(CacheEvent::PersistentWritten("profile", r)),
    )
}

pub fn write_library_cache(steamid3: u32, cached: CachedLibrary) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_library_cache(steamid3, &cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::Cache(CacheEvent::PersistentWritten("library", r)),
    )
}

pub fn write_game_cache(entry: GameCacheEntry) -> Task<crate::Message> {
    let app_id = entry.app_id;
    Task::perform(
        async move {
            cache::write_game_cache(&entry)
                .await
                .map_err(|e| e.to_string())
        },
        move |result| crate::Message::Cache(CacheEvent::GameWritten { app_id, result }),
    )
}

pub fn write_game_summary(steamid3: u32, entry: GameSummaryCache) -> Task<crate::Message> {
    let app_id = entry.app_id;
    Task::perform(
        async move {
            cache::store::write_game_summary(steamid3, &entry)
                .await
                .map_err(|e| e.to_string())
        },
        move |result| crate::Message::Cache(CacheEvent::GameWritten { app_id, result }),
    )
}

pub fn invalidate_game_cache(app_id: u32, name: String) -> Task<crate::Message> {
    Task::perform(
        async move {
            let result = cache::store::delete_game_cache_dir(app_id)
                .await
                .map_err(|e| e.to_string());
            crate::capsule_cache::purge_for_app(app_id).await;
            result
        },
        move |result| {
            crate::Message::Cache(CacheEvent::GameInvalidated {
                app_id,
                name: name.clone(),
                result,
            })
        },
    )
}

pub fn write_no_ach_cache(snapshot: NoAchievementsCache) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_no_achievements_cache(&snapshot)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::Cache(CacheEvent::NoAchWritten(r)),
    )
}

pub fn classify_games(
    games: Vec<GameSummary>,
    steam_root: PathBuf,
    steamid3: u64,
) -> Task<crate::Message> {
    Task::perform(
        async move { cache::classify_games(&games, &steam_root, steamid3).await },
        |r| crate::Message::Cache(CacheEvent::Classified(r)),
    )
}
