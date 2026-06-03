use std::path::PathBuf;

/// Falls back to the process working directory when the home env var
/// is absent — keeps boot infallible on unusual system configurations.
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

pub fn user_game_achievements_path(steamid3: u32, app_id: u32) -> PathBuf {
    user_game_dir(steamid3, app_id).join("achievements.json")
}

pub fn shared_game_icons_dir(app_id: u32) -> PathBuf {
    cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("icons")
}

pub(crate) fn legacy_profile_path() -> PathBuf {
    cache_dir().join("profile.json")
}

pub(crate) fn legacy_library_path() -> PathBuf {
    cache_dir().join("library.json")
}

pub(crate) fn legacy_game_dir(app_id: u32) -> PathBuf {
    cache_dir().join("games").join(app_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_profile_path_contains_steamid3() {
        let p = user_profile_path(123456789);
        let s = p.to_string_lossy();
        assert!(
            s.contains("users/123456789/profile.json")
                || s.contains("users\\123456789\\profile.json")
        );
    }

    #[test]
    fn user_library_path_contains_steamid3() {
        let p = user_library_path(123456789);
        let s = p.to_string_lossy();
        assert!(
            s.contains("users/123456789/library.json")
                || s.contains("users\\123456789\\library.json")
        );
    }

    #[test]
    fn user_game_summary_path_correct() {
        let p = user_game_summary_path(123456789, 570);
        let s = p.to_string_lossy();
        assert!(
            s.contains("users/123456789/games/570/summary.json")
                || s.contains("users\\123456789\\games\\570\\summary.json")
        );
    }

    #[test]
    fn user_game_achievements_path_correct() {
        let p = user_game_achievements_path(123456789, 570);
        let s = p.to_string_lossy();
        assert!(
            s.contains("users/123456789/games/570/achievements.json")
                || s.contains("users\\123456789\\games\\570\\achievements.json")
        );
    }

    #[test]
    fn shared_game_icons_dir_not_in_users() {
        let p = shared_game_icons_dir(570);
        let s = p.to_string_lossy();
        assert!(
            !s.contains("users"),
            "icons dir must be shared, not per-user"
        );
        assert!(
            s.contains("games/570/icons") || s.contains("games\\570\\icons"),
            "icons dir must be under games/570/icons"
        );
    }

    #[test]
    fn legacy_profile_path_at_cache_root() {
        let p = legacy_profile_path();
        let s = p.to_string_lossy();
        assert!(
            s.ends_with("cache/profile.json") || s.ends_with("cache\\profile.json"),
            "legacy profile.json must be at cache root, got: {s}"
        );
    }
}
