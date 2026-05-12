use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::LibraryError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSummary {
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
}

/// AppIDs that ship with every Steam account but are not real games.
/// `480` is Spacewar — Valve's SteamWorks test app, present in every user's
/// library and reports valid app metadata, but has no meaningful content.
const HARDCODED_SKIP_APP_IDS: &[u32] = &[480];

pub(crate) fn enumerate_owned_games_impl(
    client: &Client,
    apply_subscribed_filter: bool,
) -> Result<Vec<GameSummary>, LibraryError> {
    let steam_root = client.steam_root().map_err(LibraryError::SteamRoot)?;

    let packageinfo_path = steam_root.join("appcache").join("packageinfo.vdf");
    let bytes = std::fs::read(&packageinfo_path).map_err(LibraryError::PackageInfoIo)?;

    let candidate_ids =
        steamlens_vdf::parse_packageinfo(&bytes).map_err(LibraryError::PackageInfoParse)?;

    let flags_map = load_appinfo_flags(&steam_root);

    let steamid3 = (client.steam_id() & 0xFFFF_FFFF) as u32;
    let localconfig_path = steam_root
        .join("userdata")
        .join(steamid3.to_string())
        .join("config")
        .join("localconfig.vdf");

    let last_played_map = std::fs::read_to_string(&localconfig_path)
        .ok()
        .map(|content| steamlens_vdf::parse_localconfig_last_played(&content))
        .unwrap_or_default();

    let mut game_summaries = Vec::new();
    let mut skipped_ownersonly: u32 = 0;
    let mut skipped_no_store: u32 = 0;

    for (app_id, change_number) in candidate_ids {
        if HARDCODED_SKIP_APP_IDS.contains(&app_id) {
            continue;
        }

        if !is_released_game(client, app_id) {
            continue;
        }

        if let Some(flags) = flags_map.get(&app_id) {
            let is_ownersonly = flags.visibility.as_deref() == Some("ownersonly");
            let no_store_presence = !flags.has_store_asset_mtime
                && !flags.has_library_assets
                && !flags.has_header_image;
            if is_ownersonly || no_store_presence {
                let reason = if is_ownersonly {
                    skipped_ownersonly += 1;
                    "visibility=ownersonly"
                } else {
                    skipped_no_store += 1;
                    "no store presence"
                };
                let name = client
                    .get_app_data(app_id, c"name")
                    .unwrap_or_else(|| "<unknown>".to_owned());
                tracing::info!(
                    app_id,
                    name = %name,
                    reason,
                    visibility = ?flags.visibility,
                    has_store_asset_mtime = flags.has_store_asset_mtime,
                    has_library_assets = flags.has_library_assets,
                    has_header_image = flags.has_header_image,
                    "library: skipped app"
                );
                continue;
            }
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

    if skipped_ownersonly + skipped_no_store > 0 {
        tracing::info!(
            ownersonly = skipped_ownersonly,
            no_store_presence = skipped_no_store,
            kept = game_summaries.len(),
            "library: appinfo filter summary"
        );
    }

    game_summaries.sort_by_key(|g| g.app_id);
    Ok(game_summaries)
}

fn load_appinfo_flags(steam_root: &std::path::Path) -> HashMap<u32, steamlens_vdf::AppFlags> {
    let path = steam_root.join("appcache").join("appinfo.vdf");
    match std::fs::read(&path) {
        Err(err) => {
            tracing::warn!(
                ?err,
                "appinfo.vdf read failed; store-presence filter disabled"
            );
            HashMap::new()
        }
        Ok(bytes) => match steamlens_vdf::parse_appinfo_flags(&bytes) {
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "appinfo.vdf parse failed; store-presence filter disabled"
                );
                HashMap::new()
            }
            Ok(map) => map,
        },
    }
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
