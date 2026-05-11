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

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub fn game_summary_path(app_id: u32) -> PathBuf {
    cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("summary.json")
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub fn game_achievements_path(app_id: u32) -> PathBuf {
    cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("achievements.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_summary_path_uses_per_game_subdir() {
        let p = game_summary_path(570);
        let s = p.to_string_lossy();
        assert!(
            s.ends_with("cache/games/570/summary.json")
                || s.ends_with("cache\\games\\570\\summary.json")
        );
    }

    #[test]
    fn game_achievements_path_uses_per_game_subdir() {
        let p = game_achievements_path(730);
        let s = p.to_string_lossy();
        assert!(
            s.ends_with("cache/games/730/achievements.json")
                || s.ends_with("cache\\games\\730\\achievements.json")
        );
    }

    #[test]
    fn summary_and_achievements_share_parent_dir() {
        let summary = game_summary_path(42);
        let achievements = game_achievements_path(42);
        assert_eq!(summary.parent(), achievements.parent());
    }
}
