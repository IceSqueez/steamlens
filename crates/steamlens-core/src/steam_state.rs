use std::path::Path;

use steamlens_vdf::parse_text;

/// `None` on missing key / parse failure / missing file. Older Steam
/// clients may omit the `Software/Valve/Steam` nesting — callers MUST
/// NOT dirty the cache solely because `LastPlayed` is unavailable.
pub fn read_last_played(steam_root: &Path, steamid3: u64, app_id: u32) -> Option<u64> {
    let vdf_path = steam_root
        .join("userdata")
        .join(steamid3.to_string())
        .join("config/localconfig.vdf");

    let content = std::fs::read_to_string(&vdf_path).ok()?;
    let root = parse_text(&content).ok()?;

    let last_played_str = root
        .path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])?
        .get(&app_id.to_string())?
        .get("LastPlayed")?
        .as_str()?;

    last_played_str.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_localconfig(dir, 111721205, 105600, 1777926953);
        let ts = read_last_played(dir, 111721205, 105600).unwrap();
        assert_eq!(ts, 1777926953);
    }

    #[test]
    fn read_last_played_app_id_not_in_apps_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_localconfig(dir, 111721205, 105600, 1777926953);
        assert!(read_last_played(dir, 111721205, 99999).is_none());
    }

    #[test]
    fn read_last_played_missing_file_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        assert!(read_last_played(dir, 111721205, 105600).is_none());
    }
}
