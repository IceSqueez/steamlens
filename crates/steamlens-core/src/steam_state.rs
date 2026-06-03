use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use steamlens_vdf::{AppLocalState, parse_localconfig_states};

pub type SteamAppState = AppLocalState;

pub fn read_steam_state(
    steam_root: &Path,
    account_id: u64,
) -> (HashMap<u32, SteamAppState>, Option<SystemTime>) {
    let path = localconfig_path(steam_root, account_id);
    let mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let states = std::fs::read_to_string(&path)
        .ok()
        .map(|content| parse_localconfig_states(&content))
        .unwrap_or_default();
    (states, mtime)
}

pub fn read_steam_state_mtime(steam_root: &Path, account_id: u64) -> Option<SystemTime> {
    let path = localconfig_path(steam_root, account_id);
    std::fs::metadata(&path).ok()?.modified().ok()
}

fn localconfig_path(steam_root: &Path, account_id: u64) -> PathBuf {
    steam_root
        .join("userdata")
        .join(account_id.to_string())
        .join("config")
        .join("localconfig.vdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn write_localconfig(steam_root: &Path, account_id: u64, body: &str) {
        let config_dir = steam_root
            .join("userdata")
            .join(account_id.to_string())
            .join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let content = format!(
            r#""UserLocalConfigStore"
{{
    "Software"
    {{
        "Valve"
        {{
            "Steam"
            {{
                "apps"
                {{
{body}
                }}
            }}
        }}
    }}
}}"#
        );
        std::fs::write(config_dir.join("localconfig.vdf"), content).unwrap();
    }

    #[test]
    fn read_steam_state_picks_up_last_played_and_playtime() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_localconfig(
            tmp.path(),
            111721205,
            r#"                    "105600"
                    {
                        "LastPlayed"  "1777926953"
                        "Playtime"    "120"
                    }"#,
        );
        let (states, mtime) = read_steam_state(tmp.path(), 111721205);
        let entry = states.get(&105600).copied().unwrap();
        assert_eq!(entry.last_played, Some(1_777_926_953));
        assert_eq!(entry.playtime_minutes, Some(120));
        assert!(mtime.is_some());
    }

    #[test]
    fn read_steam_state_missing_file_returns_empty_with_none_mtime() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (states, mtime) = read_steam_state(tmp.path(), 111721205);
        assert!(states.is_empty());
        assert!(mtime.is_none());
    }

    #[test]
    fn read_steam_state_mtime_only_reports_change_independent_of_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_localconfig(
            tmp.path(),
            1,
            r#"                    "1" { "LastPlayed" "100" }"#,
        );
        let first = read_steam_state_mtime(tmp.path(), 1).unwrap();
        thread::sleep(Duration::from_millis(20));
        write_localconfig(
            tmp.path(),
            1,
            r#"                    "1" { "LastPlayed" "200" }"#,
        );
        let second = read_steam_state_mtime(tmp.path(), 1).unwrap();
        assert!(second > first, "mtime must advance on rewrite");
    }
}
