use std::path::PathBuf;

use iced::Task;

use crate::cache::types::GameSummaryCache;
use crate::cache::{self, CachedLibrary, GameCacheEntry, NoAchievementsCache};
use steamlens_core::GameSummary;

pub fn load_library_cache() -> Task<crate::Message> {
    Task::perform(
        async { cache::load_library_cache().await },
        crate::Message::LibraryCacheLoaded,
    )
}

pub fn load_profile_cache() -> Task<crate::Message> {
    Task::perform(
        async { cache::load_profile_cache().await },
        crate::Message::ProfileCacheLoaded,
    )
}

pub fn write_profile_cache(cached: cache::CachedProfile) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_profile_cache(&cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::PersistentCacheWritten("profile", r),
    )
}

pub fn write_library_cache(cached: CachedLibrary) -> Task<crate::Message> {
    Task::perform(
        async move {
            cache::write_library_cache(&cached)
                .await
                .map_err(|e| e.to_string())
        },
        |r| crate::Message::PersistentCacheWritten("library", r),
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
        move |result| crate::Message::CacheWritten { app_id, result },
    )
}

pub fn write_game_summary(entry: GameSummaryCache) -> Task<crate::Message> {
    let app_id = entry.app_id;
    Task::perform(
        async move {
            cache::store::write_game_summary(&entry)
                .await
                .map_err(|e| e.to_string())
        },
        move |result| crate::Message::CacheWritten { app_id, result },
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
        move |result| crate::Message::CacheInvalidated {
            app_id,
            name: name.clone(),
            result,
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
        crate::Message::NoAchCacheWritten,
    )
}

pub fn classify_games(
    games: Vec<GameSummary>,
    steam_root: PathBuf,
    steamid3: u64,
) -> Task<crate::Message> {
    Task::perform(
        async move { cache::classify_games(&games, &steam_root, steamid3).await },
        crate::Message::CacheClassified,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_library_cache_builds() {
        let _: Task<crate::Message> = load_library_cache();
    }

    #[test]
    fn load_profile_cache_builds() {
        let _: Task<crate::Message> = load_profile_cache();
    }

    #[test]
    fn classify_games_builds() {
        let _: Task<crate::Message> =
            classify_games(Vec::new(), std::path::PathBuf::from("/tmp/steam"), 0);
    }
}
