use std::path::PathBuf;

use iced::Task;

use crate::cache::{self, CachedLibrary, GameCacheEntry, NoAchievementsCache};
use crate::settings::steamlens_root;
use steamlens_core::GameSummary;

pub fn load_no_ach_cache() -> Task<crate::Message> {
    Task::perform(
        cache::load_no_achievements_cache(),
        crate::Message::NoAchCacheLoaded,
    )
}

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

pub fn clear_all_cache() -> Task<crate::Message> {
    let cache_games_dir = steamlens_root().join("cache").join("games");
    let cache_images_dir = steamlens_root().join("cache").join("images");
    Task::perform(
        async move {
            let _ = tokio::fs::remove_dir_all(&cache_games_dir).await;
            let _ = tokio::fs::remove_dir_all(&cache_images_dir).await;
        },
        |()| crate::Message::ToastRequest("Cache cleared".to_owned()),
    )
}

pub fn clear_game_cache(cache_path: PathBuf, game_name: String) -> Task<crate::Message> {
    Task::perform(
        async move {
            let _ = tokio::fs::remove_file(&cache_path).await;
            game_name
        },
        |name| crate::Message::ToastRequest(format!("Cache cleared for {name}")),
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
    fn load_no_ach_cache_builds() {
        let _: Task<crate::Message> = load_no_ach_cache();
    }

    #[test]
    fn load_library_cache_builds() {
        let _: Task<crate::Message> = load_library_cache();
    }

    #[test]
    fn load_profile_cache_builds() {
        let _: Task<crate::Message> = load_profile_cache();
    }

    #[test]
    fn clear_all_cache_builds() {
        let _: Task<crate::Message> = clear_all_cache();
    }

    #[test]
    fn clear_game_cache_builds() {
        let _: Task<crate::Message> = clear_game_cache(
            std::path::PathBuf::from("/tmp/440.json"),
            "Test Game".to_owned(),
        );
    }

    #[test]
    fn classify_games_builds() {
        let _: Task<crate::Message> =
            classify_games(Vec::new(), std::path::PathBuf::from("/tmp/steam"), 0);
    }
}
