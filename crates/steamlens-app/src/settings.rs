use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::library::types::LibrarySort;
use crate::manager::types::{AchievementFilter, AchievementSort, RarityFilter};

const CURRENT_SETTINGS_VERSION: u32 = 1;

/// Returns `$HOME/.steamlens/` on all platforms.
///
/// Falls back to the process working directory when the home environment
/// variable is absent — this keeps the function infallible and avoids
/// panicking at startup on unusual system configurations.
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

/// Returns the path to `settings.toml` inside the SteamLens root directory.
pub fn settings_path() -> PathBuf {
    steamlens_root().join("settings.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiSettings {
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_window_width() -> f32 {
    1280.0
}

fn default_window_height() -> f32 {
    800.0
}

fn default_theme() -> String {
    "dracula".to_owned()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            window_width: default_window_width(),
            window_height: default_window_height(),
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarySettings {
    #[serde(default)]
    pub search: String,
    #[serde(default = "default_library_sort")]
    pub sort: LibrarySort,
}

fn default_library_sort() -> LibrarySort {
    LibrarySort::LastPlayed
}

impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            search: String::new(),
            sort: default_library_sort(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManagerSettings {
    #[serde(default)]
    pub search: String,
    #[serde(default = "default_achievement_filter")]
    pub filter: AchievementFilter,
    #[serde(default = "default_achievement_sort")]
    pub sort: AchievementSort,
    #[serde(default = "default_rarity_filter")]
    pub rarity_filter: RarityFilter,
}

fn default_achievement_filter() -> AchievementFilter {
    AchievementFilter::All
}

fn default_achievement_sort() -> AchievementSort {
    AchievementSort::UnlockChance
}

fn default_rarity_filter() -> RarityFilter {
    RarityFilter::All
}

impl Default for ManagerSettings {
    fn default() -> Self {
        Self {
            search: String::new(),
            filter: default_achievement_filter(),
            sort: default_achievement_sort(),
            rarity_filter: default_rarity_filter(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default)]
    pub library: LibrarySettings,
    #[serde(default)]
    pub manager: ManagerSettings,
}

fn default_schema_version() -> u32 {
    CURRENT_SETTINGS_VERSION
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SETTINGS_VERSION,
            ui: UiSettings::default(),
            library: LibrarySettings::default(),
            manager: ManagerSettings::default(),
        }
    }
}

/// Loads settings from disk, returning defaults on any error.
///
/// Errors that trigger fallback to defaults:
/// - File missing or not readable
/// - TOML parse failure (corrupted content, wrong types)
/// - `schema_version` mismatch (future or past format)
/// - Path points to a directory instead of a file
///
/// All error conditions are logged at `warn` level. The function never panics.
pub fn load_settings() -> Settings {
    let path = settings_path();

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "[steamlens] settings: could not read {}: {e}",
                path.display()
            );
            return Settings::default();
        }
    };

    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[steamlens] settings: file is not valid UTF-8: {e}");
            return Settings::default();
        }
    };

    let parsed: Settings = match toml::from_str(text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[steamlens] settings: TOML parse error: {e}");
            return Settings::default();
        }
    };

    if parsed.schema_version != CURRENT_SETTINGS_VERSION {
        eprintln!(
            "[steamlens] settings: schema version {} does not match expected {}; using defaults",
            parsed.schema_version, CURRENT_SETTINGS_VERSION
        );
        return Settings::default();
    }

    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_from_path(path: &std::path::Path) -> Settings {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "[steamlens] settings: could not read {}: {e}",
                    path.display()
                );
                return Settings::default();
            }
        };

        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[steamlens] settings: file is not valid UTF-8: {e}");
                return Settings::default();
            }
        };

        let parsed: Settings = match toml::from_str(text) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[steamlens] settings: TOML parse error: {e}");
                return Settings::default();
            }
        };

        if parsed.schema_version != CURRENT_SETTINGS_VERSION {
            eprintln!(
                "[steamlens] settings: schema version {} does not match expected {}; using defaults",
                parsed.schema_version, CURRENT_SETTINGS_VERSION
            );
            return Settings::default();
        }

        parsed
    }

    fn round_trip(settings: &Settings) -> Settings {
        let text = toml::to_string_pretty(settings).expect("serialize");
        toml::from_str(&text).expect("deserialize")
    }

    #[test]
    fn default_settings_round_trip() {
        let original = Settings::default();
        let restored = round_trip(&original);
        assert_eq!(original, restored);
    }

    #[test]
    fn non_default_settings_round_trip() {
        let original = Settings {
            schema_version: 1,
            ui: UiSettings {
                window_width: 1920.0,
                window_height: 1080.0,
                theme: "dracula".to_owned(),
            },
            library: LibrarySettings {
                search: "terra".to_owned(),
                sort: LibrarySort::NameAsc,
            },
            manager: ManagerSettings {
                search: String::new(),
                filter: AchievementFilter::Locked,
                sort: AchievementSort::Name,
                rarity_filter: RarityFilter::Legendary,
            },
        };
        let restored = round_trip(&original);
        assert_eq!(original, restored);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_missing_999999.toml");
        let _ = std::fs::remove_file(&tmp);
        let result = load_from_path(&tmp);
        assert_eq!(result, Settings::default());
    }

    #[test]
    fn corrupted_file_returns_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_corrupt_999999.toml");
        std::fs::write(&tmp, b"not valid toml ][[[").expect("write");
        let result = load_from_path(&tmp);
        assert_eq!(result, Settings::default());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn binary_garbage_returns_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_binary_999999.toml");
        std::fs::write(&tmp, [0xFF, 0xFE, 0x00, 0x01, 0xAB, 0xCD]).expect("write");
        let result = load_from_path(&tmp);
        assert_eq!(result, Settings::default());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn path_is_directory_returns_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_dir_999999");
        std::fs::create_dir_all(&tmp).expect("mkdir");
        let result = load_from_path(&tmp);
        assert_eq!(result, Settings::default());
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn schema_version_mismatch_returns_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_schema_999999.toml");
        let bad_version = "schema_version = 99\n[ui]\nwindow_width = 1920.0\n";
        std::fs::write(&tmp, bad_version).expect("write");
        let result = load_from_path(&tmp);
        assert_eq!(result, Settings::default());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn partial_settings_fills_in_defaults() {
        let tmp = std::env::temp_dir().join("steamlens_test_partial_999999.toml");
        let partial = "schema_version = 1\n[library]\nsort = \"name_asc\"\n";
        std::fs::write(&tmp, partial).expect("write");
        let result = load_from_path(&tmp);
        assert_eq!(result.library.sort, LibrarySort::NameAsc);
        assert_eq!(result.ui.window_width, 1280.0);
        assert_eq!(result.manager.filter, AchievementFilter::All);
        let _ = std::fs::remove_file(&tmp);
    }
}
