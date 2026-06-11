use std::path::PathBuf;

use iced::Task;

use crate::cache::{self, CacheEvent, CachedLibrary, GameCacheEntry, NoAchievementsCache};
use steamlens_core::GameSummary;

pub fn load_library_cache(account_id: u32) -> Task<crate::Message> {
    Task::perform(
        async move { cache::load_library_cache(account_id).await },
        |c| crate::Message::Cache(CacheEvent::LibraryLoaded(c)),
    )
}

pub fn load_profile_cache(account_id: u32) -> Task<crate::Message> {
    Task::perform(
        async move { cache::load_profile_cache(account_id).await },
        |c| crate::Message::Cache(CacheEvent::ProfileLoaded(c)),
    )
}

pub fn write_profile_cache(account_id: u32, cached: cache::CachedProfile) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_profile_cache(account_id, &cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::Cache(CacheEvent::PersistentWritten("profile", r)),
    )
}

pub fn write_library_cache(account_id: u32, cached: CachedLibrary) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_library_cache(account_id, &cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::Cache(CacheEvent::PersistentWritten("library", r)),
    )
}

pub fn write_game_cache(account_id: u32, entry: GameCacheEntry) -> Task<crate::Message> {
    let app_id = entry.app_id;
    Task::perform(
        async move {
            cache::write_game_cache(account_id, &entry)
                .await
                .map_err(|e| e.to_string())
        },
        move |result| crate::Message::Cache(CacheEvent::GameWritten { app_id, result }),
    )
}

pub fn invalidate_game_cache(account_id: u32, app_id: u32, name: String) -> Task<crate::Message> {
    Task::perform(
        async move {
            let result = cache::store::delete_game_cache_dir(account_id, app_id)
                .await
                .map_err(|e| e.to_string());
            crate::capsule_cache::purge_for_app(app_id).await;
            result
        },
        move |result| {
            crate::Message::Cache(CacheEvent::GameInvalidated {
                app_id,
                name,
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
        |r| crate::Message::Cache(CacheEvent::NoAchievementsWritten(r)),
    )
}

pub fn classify_games(
    games: Vec<GameSummary>,
    steam_root: PathBuf,
    account_id: u32,
) -> Task<crate::Message> {
    Task::perform(
        async move { cache::classify_games(&games, &steam_root, account_id).await },
        |r| crate::Message::Cache(CacheEvent::Classified(r)),
    )
}
