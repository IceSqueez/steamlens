use std::path::PathBuf;

pub fn steamlens_root() -> PathBuf {
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var("HOME").unwrap_or_default();

    #[cfg(target_os = "windows")]
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| {
        let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
        let path = std::env::var("HOMEPATH").unwrap_or_default();
        format!("{drive}{path}")
    });

    if home.is_empty() {
        PathBuf::from(".steamlens")
    } else {
        PathBuf::from(home).join(".steamlens")
    }
}

pub fn settings_path() -> PathBuf {
    steamlens_root().join("settings.toml")
}

pub fn cache_dir() -> PathBuf {
    steamlens_root().join("cache")
}

pub fn capsules_dir() -> PathBuf {
    steamlens_root().join("capsules")
}

pub fn log_path() -> PathBuf {
    steamlens_root().join("steamlens.log")
}

pub fn no_achievements_path() -> PathBuf {
    cache_dir().join("no_achievements.json")
}

pub fn users_dir() -> PathBuf {
    cache_dir().join("users")
}

pub fn user_dir(steamid3: u32) -> PathBuf {
    users_dir().join(steamid3.to_string())
}

pub fn user_profile_path(steamid3: u32) -> PathBuf {
    user_dir(steamid3).join("profile.json")
}

pub fn user_library_path(steamid3: u32) -> PathBuf {
    user_dir(steamid3).join("library.json")
}

pub fn user_game_dir(steamid3: u32, app_id: u32) -> PathBuf {
    user_dir(steamid3).join("games").join(app_id.to_string())
}

pub fn user_game_summary_path(steamid3: u32, app_id: u32) -> PathBuf {
    user_game_dir(steamid3, app_id).join("summary.json")
}

pub fn user_game_cache_path(steamid3: u32, app_id: u32) -> PathBuf {
    user_game_dir(steamid3, app_id).join("cache.json")
}

pub fn shared_game_icons_dir(app_id: u32) -> PathBuf {
    cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("icons")
}
