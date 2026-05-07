use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::LibraryError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSummary {
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
}

pub(crate) fn enumerate_owned_games_impl(
    client: &Client,
    apply_subscribed_filter: bool,
) -> Result<Vec<GameSummary>, LibraryError> {
    let steam_root = client.steam_root().map_err(LibraryError::SteamRoot)?;

    let packageinfo_path = steam_root.join("appcache/packageinfo.vdf");
    let bytes = std::fs::read(&packageinfo_path).map_err(LibraryError::PackageInfoIo)?;

    let candidate_ids =
        steamlens_vdf::parse_packageinfo(&bytes).map_err(LibraryError::PackageInfoParse)?;

    let steamid3 = (client.steam_id() & 0xFFFF_FFFF) as u32;
    let localconfig_path = steam_root
        .join("userdata")
        .join(steamid3.to_string())
        .join("config/localconfig.vdf");

    let last_played_map = std::fs::read_to_string(&localconfig_path)
        .ok()
        .map(|content| steamlens_vdf::parse_localconfig_last_played(&content))
        .unwrap_or_default();

    let mut game_summaries = Vec::new();

    for (app_id, change_number) in candidate_ids {
        if !is_released_game(client, app_id) {
            continue;
        }

        if apply_subscribed_filter && !client.is_subscribed_app(app_id) {
            continue;
        }

        game_summaries.push(GameSummary {
            app_id,
            change_number,
            last_played: last_played_map.get(&app_id).copied(),
        });
    }

    game_summaries.sort_by_key(|g| g.app_id);
    Ok(game_summaries)
}

fn is_released_game(client: &Client, app_id: u32) -> bool {
    match client.app_type(app_id) {
        Some(t) if t.eq_ignore_ascii_case("game") => {}
        _ => return false,
    }

    if let Some(state) = client.get_app_data(app_id, c"ReleaseState")
        && !state.eq_ignore_ascii_case("released")
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerated_game_serde_roundtrip() {
        let game_summary = GameSummary {
            app_id: 12345,
            change_number: 42,
            last_played: Some(1_700_000_000),
        };

        let json = serde_json::to_string(&game_summary).expect("serialize");
        let restored: GameSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, game_summary);
    }

    #[test]
    fn enumerated_game_serde_no_last_played() {
        let game_summary = GameSummary {
            app_id: 99999,
            change_number: 0,
            last_played: None,
        };

        let json = serde_json::to_string(&game_summary).expect("serialize");
        let restored: GameSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, game_summary);
    }
}
