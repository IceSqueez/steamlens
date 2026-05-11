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

pub fn no_achievements_path() -> PathBuf {
    cache_dir().join("no_achievements.json")
}
