use std::collections::HashMap;
use std::path::Path;

use steamlens_vdf::parse_text;

/// Snapshot of the fields needed to detect whether a game's Steam state has
/// changed since the last cache write.
pub struct ManifestState {
    /// Unix timestamp from `AppState.LastUpdated` in the appmanifest ACF.
    pub last_updated: u64,
    /// Steam build identifier string from `AppState.buildid`.
    pub build_id: String,
}

/// Read `LastUpdated` and `buildid` from an appmanifest ACF file.
///
/// Returns `None` on file-missing, I/O error, or VDF parse failure.
///
/// **Rule M (RFC-002 §7.1):** a missing manifest is a normal, expected state
/// for uninstalled games — callers MUST treat `None` as "no invalidation
/// signal", not as an error.  Do not log warnings or return an error on
/// `NotFound`; simply return `None` and let the caller preserve the existing
/// cache entry.
pub fn read_manifest_state(manifest_path: &Path) -> Option<ManifestState> {
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let root = parse_text(&content).ok()?;
    let app_state = root.get("AppState")?;

    let last_updated = app_state
        .get("LastUpdated")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let build_id = app_state
        .get("buildid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    Some(ManifestState {
        last_updated,
        build_id,
    })
}

/// Read `LastPlayed` for one app from the active user's `localconfig.vdf`.
///
/// Walks `UserLocalConfigStore → Software → Valve → Steam → apps → <app_id>
/// → LastPlayed` (five levels deep, as verified on Linux with Steam client
/// version current as of 2026-05-04).
///
/// Returns `None` on any missing key, parse failure, or missing file.  Per
/// RFC-002 Risk #1, older Steam clients may omit the `Software/Valve/Steam`
/// nesting; `None` is the correct and safe fallback in that case — callers
/// must not dirty the cache solely because `LastPlayed` is unavailable.
pub fn read_last_played(steam_root: &Path, steamid3: u64, app_id: u32) -> Option<u64> {
    let vdf_path = steam_root
        .join("userdata")
        .join(steamid3.to_string())
        .join("config/localconfig.vdf");

    let content = std::fs::read_to_string(&vdf_path).ok()?;
    let root = parse_text(&content).ok()?;

    let last_played_str = root
        .get("UserLocalConfigStore")?
        .get("Software")?
        .get("Valve")?
        .get("Steam")?
        .get("apps")?
        .get(&app_id.to_string())?
        .get("LastPlayed")?
        .as_str()?;

    last_played_str.parse::<u64>().ok()
}

/// Parse `localconfig.vdf` once and return all `LastPlayed` timestamps keyed by
/// app_id. Returns an empty `HashMap` on missing file or parse failure (callers
/// must treat absent entries as `None`-equivalent — never as a dirty signal).
///
/// Prefer this over calling [`read_last_played`] in a loop: the localconfig
/// file is typically ~500 KB and re-parsing it per app multiplies wall-clock
/// time linearly with library size.
pub fn read_all_last_played(steam_root: &Path, steamid3: u64) -> HashMap<u32, u64> {
    let vdf_path = steam_root
        .join("userdata")
        .join(steamid3.to_string())
        .join("config/localconfig.vdf");

    let Ok(content) = std::fs::read_to_string(&vdf_path) else {
        return HashMap::new();
    };
    let Ok(root) = parse_text(&content) else {
        return HashMap::new();
    };

    let Some(apps) = root
        .get("UserLocalConfigStore")
        .and_then(|v| v.get("Software"))
        .and_then(|v| v.get("Valve"))
        .and_then(|v| v.get("Steam"))
        .and_then(|v| v.get("apps"))
    else {
        return HashMap::new();
    };

    let mut map = HashMap::new();
    if let Some(pairs) = apps.as_block() {
        for (app_id_str, app_node) in pairs {
            let Ok(app_id) = app_id_str.parse::<u32>() else {
                continue;
            };
            let Some(lp_str) = app_node.get("LastPlayed").and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(lp) = lp_str.parse::<u64>() else {
                continue;
            };
            map.insert(app_id, lp);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_steam_state_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn read_manifest_state_happy_path() {
        let dir = tempdir();
        let path = write_file(
            &dir,
            "appmanifest_105600.acf",
            r#"
"AppState"
{
    "appid"       "105600"
    "name"        "Terraria"
    "buildid"     "13579246"
    "LastUpdated" "1777629959"
}
"#,
        );
        let state = read_manifest_state(&path).unwrap();
        assert_eq!(state.last_updated, 1777629959);
        assert_eq!(state.build_id, "13579246");
    }

    #[test]
    fn read_manifest_state_nonexistent_path_returns_none() {
        let dir = tempdir();
        let missing = dir.join("appmanifest_999999.acf");
        assert!(
            read_manifest_state(&missing).is_none(),
            "Rule M: missing manifest must return None, not an error"
        );
    }

    #[test]
    fn read_manifest_state_malformed_vdf_returns_none() {
        let dir = tempdir();
        let path = write_file(&dir, "appmanifest_bad.acf", "this is not { valid } vdf !!!");
        assert!(read_manifest_state(&path).is_none());
    }

    #[test]
    fn read_manifest_state_missing_last_updated_field_returns_zero() {
        let dir = tempdir();
        let path = write_file(
            &dir,
            "appmanifest_nots.acf",
            r#"
"AppState"
{
    "appid"   "570"
    "name"    "Dota 2"
    "buildid" "99887766"
}
"#,
        );
        let state = read_manifest_state(&path).unwrap();
        assert_eq!(
            state.last_updated, 0,
            "absent LastUpdated must default to 0"
        );
        assert_eq!(state.build_id, "99887766");
    }

    fn write_localconfig(
        steam_root: &std::path::Path,
        steamid3: u64,
        app_id: u32,
        last_played: u64,
    ) {
        let config_dir = steam_root
            .join("userdata")
            .join(steamid3.to_string())
            .join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let content = format!(
            r#"
"UserLocalConfigStore"
{{
    "Software"
    {{
        "Valve"
        {{
            "Steam"
            {{
                "apps"
                {{
                    "{app_id}"
                    {{
                        "LastPlayed"  "{last_played}"
                    }}
                }}
            }}
        }}
    }}
}}
"#
        );
        std::fs::write(config_dir.join("localconfig.vdf"), content).unwrap();
    }

    #[test]
    fn read_last_played_happy_path() {
        let dir = tempdir();
        write_localconfig(&dir, 111721205, 105600, 1777926953);
        let ts = read_last_played(&dir, 111721205, 105600).unwrap();
        assert_eq!(ts, 1777926953);
    }

    #[test]
    fn read_last_played_app_id_not_in_apps_returns_none() {
        let dir = tempdir();
        write_localconfig(&dir, 111721205, 105600, 1777926953);
        assert!(read_last_played(&dir, 111721205, 99999).is_none());
    }

    #[test]
    fn read_last_played_missing_file_returns_none() {
        let dir = tempdir();
        assert!(read_last_played(&dir, 111721205, 105600).is_none());
    }

    fn write_localconfig_multi(steam_root: &std::path::Path, steamid3: u64, apps: &[(u32, u64)]) {
        let config_dir = steam_root
            .join("userdata")
            .join(steamid3.to_string())
            .join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let mut apps_block = String::new();
        for (app_id, lp) in apps {
            apps_block.push_str(&format!(
                "                    \"{app_id}\"\n                    {{\n                        \"LastPlayed\"  \"{lp}\"\n                    }}\n"
            ));
        }
        let content = format!(
            "\"UserLocalConfigStore\"\n{{\n    \"Software\"\n    {{\n        \"Valve\"\n        {{\n            \"Steam\"\n            {{\n                \"apps\"\n                {{\n{apps_block}                }}\n            }}\n        }}\n    }}\n}}\n"
        );
        std::fs::write(config_dir.join("localconfig.vdf"), content).unwrap();
    }

    #[test]
    fn read_all_last_played_happy_path_three_apps() {
        let dir = tempdir();
        write_localconfig_multi(
            &dir,
            111721205,
            &[(105600, 1777926953), (570, 1700000000), (440, 1650000000)],
        );
        let map = read_all_last_played(&dir, 111721205);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&105600), Some(&1777926953));
        assert_eq!(map.get(&570), Some(&1700000000));
        assert_eq!(map.get(&440), Some(&1650000000));
    }

    #[test]
    fn read_all_last_played_missing_file_returns_empty_map() {
        let dir = tempdir();
        let map = read_all_last_played(&dir, 111721205);
        assert!(map.is_empty());
    }

    #[test]
    fn read_all_last_played_skips_apps_without_lastplayed_key() {
        let dir = tempdir();
        let config_dir = dir.join("userdata").join("111721205").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let content = "\"UserLocalConfigStore\"\n{\n    \"Software\"\n    {\n        \"Valve\"\n        {\n            \"Steam\"\n            {\n                \"apps\"\n                {\n                    \"105600\"\n                    {\n                        \"LastPlayed\"  \"1777926953\"\n                    }\n                    \"570\"\n                    {\n                        \"SomeOther\"  \"value\"\n                    }\n                }\n            }\n        }\n    }\n}\n";
        std::fs::write(config_dir.join("localconfig.vdf"), content).unwrap();
        let map = read_all_last_played(&dir, 111721205);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&105600));
        assert!(!map.contains_key(&570));
    }
}
