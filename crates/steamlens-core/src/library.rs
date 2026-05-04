use std::io;
use std::path::{Path, PathBuf};

use steamlens_vdf::parse_text;

use crate::error::LibraryScanError;

/// A summary of a single installed Steam game, as read entirely from local disk.
#[derive(Debug, Clone, PartialEq)]
pub struct GameSummary {
    pub app_id: u32,
    pub name: String,
    /// Unix timestamp of the last play session, or `None` if the game has
    /// never been played (`LastPlayed` was absent or `"0"`).
    pub last_played: Option<u32>,
    /// Number of achievements defined in the local schema cache.  Zero when
    /// the schema file is missing or cannot be parsed — this is not an error.
    pub achievement_count: u32,
}

/// Scan all installed Steam libraries and return a list of games that have
/// at least one achievement defined in the local schema cache.
///
/// Each library is discovered via `libraryfolders.vdf`.  If that file is
/// missing, the default platform Steam root is used as the sole library.
/// Per-game parse failures are silently skipped; only a total I/O failure
/// reading `libraryfolders.vdf` (AND the fallback) propagates as an error.
pub fn scan_installed_games() -> Result<Vec<GameSummary>, LibraryScanError> {
    let root = default_steam_root();
    scan_with_steam_root(&root)
}

/// Testable entry point: accepts an explicit Steam root path instead of the
/// platform-default one.  The public `scan_installed_games` is a thin wrapper.
pub fn scan_with_steam_root(steam_root: &Path) -> Result<Vec<GameSummary>, LibraryScanError> {
    let library_paths = discover_library_paths(steam_root)?;
    let manifests = enumerate_appmanifests(&library_paths);

    let mut results = Vec::new();
    for manifest_path in manifests {
        if let Some(mut summary) = parse_appmanifest(&manifest_path) {
            summary.achievement_count = count_achievements_for_app(steam_root, summary.app_id);
            if summary.achievement_count > 0 {
                results.push(summary);
            }
        }
    }

    results.sort_by_key(|g| g.app_id);
    Ok(results)
}

fn default_steam_root() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".local/share/Steam")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join("Library/Application Support/Steam")
    }
    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| r"C:\Program Files (x86)".to_owned());
        PathBuf::from(program_files).join("Steam")
    }
}

fn discover_library_paths(steam_root: &Path) -> Result<Vec<PathBuf>, LibraryScanError> {
    let vdf_path = steam_root.join("steamapps/libraryfolders.vdf");

    let content = match std::fs::read_to_string(&vdf_path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(vec![steam_root.to_path_buf()]);
        }
        Err(e) => return Err(LibraryScanError::LibraryFoldersIo(e)),
    };

    let root = match parse_text(&content) {
        Ok(v) => v,
        Err(_) => return Ok(vec![steam_root.to_path_buf()]),
    };

    let mut paths = Vec::new();

    let lf = match root.get("libraryfolders") {
        Some(v) => v,
        None => return Ok(vec![steam_root.to_path_buf()]),
    };

    let pairs = match lf.as_block() {
        Some(p) => p,
        None => return Ok(vec![steam_root.to_path_buf()]),
    };

    for (key, value) in pairs {
        if key.parse::<u64>().is_ok()
            && let Some(path_str) = value.get("path").and_then(|v| v.as_str())
        {
            let p = PathBuf::from(path_str);
            if !path_str.is_empty() {
                paths.push(p);
            }
        }
    }

    if paths.is_empty() {
        return Err(LibraryScanError::NoLibrariesFound);
    }

    Ok(paths)
}

fn enumerate_appmanifests(library_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();

    for lib_path in library_paths {
        let steamapps = lib_path.join("steamapps");
        let entries = match std::fs::read_dir(&steamapps) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("appmanifest_") && name_str.ends_with(".acf") {
                result.push(entry.path());
            }
        }
    }

    result.sort_by(|a, b| {
        let app_id_a = extract_app_id_from_path(a);
        let app_id_b = extract_app_id_from_path(b);
        app_id_a.cmp(&app_id_b)
    });

    result
}

fn extract_app_id_from_path(path: &Path) -> u32 {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("appmanifest_"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn parse_appmanifest(path: &Path) -> Option<GameSummary> {
    let content = std::fs::read_to_string(path).ok()?;
    let root = parse_text(&content).ok()?;
    let app_state = root.get("AppState")?;

    let app_id: u32 = app_state
        .get("appid")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())?;

    let name = app_state
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())?;

    let last_played = app_state
        .get("LastPlayed")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&ts| ts != 0);

    Some(GameSummary {
        app_id,
        name,
        last_played,
        achievement_count: 0,
    })
}

fn count_achievements_for_app(steam_root: &Path, app_id: u32) -> u32 {
    let schema_path = steam_root
        .join("appcache/stats")
        .join(format!("UserGameStatsSchema_{app_id}.bin"));

    let bytes = match std::fs::read(&schema_path) {
        Ok(b) => b,
        Err(_) => return 0,
    };

    let root = match steamlens_vdf::parse(&bytes) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let stats_path = format!("{app_id}/stats");
    let stats = match root.get(&stats_path) {
        Some(v) => v,
        None => return 0,
    };

    let pairs = match stats.as_section() {
        Some(p) => p,
        None => return 0,
    };

    let mut count = 0u32;

    for stat_pair in pairs {
        let children = match stat_pair.value.as_section() {
            Some(c) => c,
            None => continue,
        };

        let type_str = children
            .iter()
            .find(|p| p.key == "type")
            .and_then(|p| p.value.as_str())
            .unwrap_or("");

        if type_str != "ACHIEVEMENTS" {
            continue;
        }

        let bits = match children
            .iter()
            .find(|p| p.key == "bits")
            .and_then(|p| p.value.as_section())
        {
            Some(b) => b,
            None => continue,
        };

        count += bits.len() as u32;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_appmanifest_extracts_fields() {
        let tmp = tempfile_appmanifest(
            r#"
"AppState"
{
    "appid"         "105600"
    "name"          "Terraria"
    "StateFlags"    "4"
    "LastPlayed"    "1700000000"
}
"#,
        );
        let summary = parse_appmanifest(&tmp).unwrap();
        assert_eq!(summary.app_id, 105600);
        assert_eq!(summary.name, "Terraria");
        assert_eq!(summary.last_played, Some(1700000000));
    }

    #[test]
    fn parse_appmanifest_last_played_zero_is_none() {
        let tmp = tempfile_appmanifest(
            r#"
"AppState"
{
    "appid"     "228980"
    "name"      "Steamworks Common Redistributables"
    "LastPlayed" "0"
}
"#,
        );
        let summary = parse_appmanifest(&tmp).unwrap();
        assert_eq!(summary.last_played, None);
    }

    #[test]
    fn parse_appmanifest_missing_last_played_is_none() {
        let tmp = tempfile_appmanifest(
            r#"
"AppState"
{
    "appid" "999"
    "name"  "TestGame"
}
"#,
        );
        let summary = parse_appmanifest(&tmp).unwrap();
        assert_eq!(summary.last_played, None);
    }

    #[test]
    fn parse_appmanifest_missing_name_returns_none() {
        let tmp = tempfile_appmanifest(
            r#"
"AppState"
{
    "appid" "999"
}
"#,
        );
        assert!(parse_appmanifest(&tmp).is_none());
    }

    #[test]
    fn count_achievements_from_inline_schema() {
        let schema_bytes = build_inline_schema(105600, 3);
        let tmp_dir = tempdir();
        let stats_dir = tmp_dir.join("appcache/stats");
        std::fs::create_dir_all(&stats_dir).unwrap();
        std::fs::write(
            stats_dir.join("UserGameStatsSchema_105600.bin"),
            &schema_bytes,
        )
        .unwrap();

        let count = count_achievements_for_app(&tmp_dir, 105600);
        assert_eq!(count, 3);
    }

    #[test]
    fn count_achievements_missing_schema_returns_zero() {
        let tmp_dir = tempdir();
        let count = count_achievements_for_app(&tmp_dir, 99999);
        assert_eq!(count, 0);
    }

    #[test]
    fn enumerate_appmanifests_sorted_by_app_id() {
        let tmp_dir = tempdir();
        let steamapps = tmp_dir.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        std::fs::write(steamapps.join("appmanifest_570.acf"), b"").unwrap();
        std::fs::write(steamapps.join("appmanifest_105600.acf"), b"").unwrap();
        std::fs::write(steamapps.join("appmanifest_400.acf"), b"").unwrap();

        let manifests = enumerate_appmanifests(&[tmp_dir.clone()]);
        let ids: Vec<u32> = manifests
            .iter()
            .map(|p| extract_app_id_from_path(p))
            .collect();
        assert_eq!(ids, vec![400, 570, 105600]);
    }

    #[test]
    fn scan_with_steam_root_filters_no_achievements() {
        let tmp_dir = tempdir();
        let steamapps = tmp_dir.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();

        std::fs::write(
            steamapps.join("appmanifest_99.acf"),
            r#""AppState" { "appid" "99" "name" "NoAch" "LastPlayed" "0" }"#,
        )
        .unwrap();

        let result = scan_with_steam_root(&tmp_dir).unwrap();
        assert!(
            result.is_empty(),
            "game without schema should be filtered out"
        );
    }

    #[test]
    fn scan_with_steam_root_includes_game_with_achievements() {
        let tmp_dir = tempdir();
        let steamapps = tmp_dir.join("steamapps");
        let stats_dir = tmp_dir.join("appcache/stats");
        std::fs::create_dir_all(&steamapps).unwrap();
        std::fs::create_dir_all(&stats_dir).unwrap();

        std::fs::write(
            steamapps.join("appmanifest_105600.acf"),
            r#""AppState" { "appid" "105600" "name" "Terraria" "LastPlayed" "1700000000" }"#,
        )
        .unwrap();

        let schema_bytes = build_inline_schema(105600, 5);
        std::fs::write(
            stats_dir.join("UserGameStatsSchema_105600.bin"),
            &schema_bytes,
        )
        .unwrap();

        let result = scan_with_steam_root(&tmp_dir).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].app_id, 105600);
        assert_eq!(result[0].name, "Terraria");
        assert_eq!(result[0].achievement_count, 5);
        assert_eq!(result[0].last_played, Some(1700000000));
    }

    #[test]
    fn scan_handles_missing_libraryfolders_falls_back_to_root() {
        let tmp_dir = tempdir();
        let steamapps = tmp_dir.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();

        let result = scan_with_steam_root(&tmp_dir).unwrap();
        assert!(result.is_empty());
    }

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn tempfile_appmanifest(content: &str) -> PathBuf {
        let dir = tempdir();
        let path = dir.join("appmanifest_test.acf");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn build_inline_schema(app_id: u32, achievement_count: u32) -> Vec<u8> {
        use steamlens_vdf::{KeyValuePair, Value};

        let mut bits = Vec::new();
        for i in 0..achievement_count {
            bits.push(KeyValuePair {
                key: i.to_string(),
                value: Value::Section(vec![KeyValuePair {
                    key: "name".to_owned(),
                    value: Value::String(format!("ACH_{i}")),
                }]),
            });
        }

        let achievements_entry = KeyValuePair {
            key: "0".to_owned(),
            value: Value::Section(vec![
                KeyValuePair {
                    key: "type".to_owned(),
                    value: Value::String("ACHIEVEMENTS".to_owned()),
                },
                KeyValuePair {
                    key: "bits".to_owned(),
                    value: Value::Section(bits),
                },
            ]),
        };

        let root = Value::Section(vec![KeyValuePair {
            key: app_id.to_string(),
            value: Value::Section(vec![KeyValuePair {
                key: "stats".to_owned(),
                value: Value::Section(vec![achievements_entry]),
            }]),
        }]);

        encode_binary_vdf(&root)
    }

    fn encode_binary_vdf(value: &steamlens_vdf::Value) -> Vec<u8> {
        let mut out = Vec::new();
        encode_section_children(value, &mut out);
        out.push(0x08); // End
        out
    }

    fn encode_section_children(section: &steamlens_vdf::Value, out: &mut Vec<u8>) {
        use steamlens_vdf::Value;
        let pairs = match section.as_section() {
            Some(p) => p,
            None => return,
        };
        for pair in pairs {
            match &pair.value {
                Value::Section(_) => {
                    out.push(0x00);
                    out.extend_from_slice(pair.key.as_bytes());
                    out.push(0x00);
                    encode_section_children(&pair.value, out);
                    out.push(0x08);
                }
                Value::String(s) => {
                    out.push(0x01);
                    out.extend_from_slice(pair.key.as_bytes());
                    out.push(0x00);
                    out.extend_from_slice(s.as_bytes());
                    out.push(0x00);
                }
                Value::Int32(n) => {
                    out.push(0x02);
                    out.extend_from_slice(pair.key.as_bytes());
                    out.push(0x00);
                    out.extend_from_slice(&n.to_le_bytes());
                }
                Value::Float32(f) => {
                    out.push(0x03);
                    out.extend_from_slice(pair.key.as_bytes());
                    out.push(0x00);
                    out.extend_from_slice(&f.to_le_bytes());
                }
                Value::UInt64(u) => {
                    out.push(0x07);
                    out.extend_from_slice(pair.key.as_bytes());
                    out.push(0x00);
                    out.extend_from_slice(&u.to_le_bytes());
                }
            }
        }
    }
}
