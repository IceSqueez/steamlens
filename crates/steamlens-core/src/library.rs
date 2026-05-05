use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::LibraryError;

/// A summary of a single Steam game as returned by the library enumeration pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSummary {
    pub app_id: u32,
    pub name: String,
    /// Unix timestamp of the last play session, or `None` if the game has
    /// never been played (`LastPlayed` was absent or `"0"`).
    pub last_played: Option<u32>,
    /// Number of achievements for this game.  Zero at enumeration time;
    /// populated later by the per-game subprocess worker.
    pub achievement_count: u32,
    /// PICS change number of the package(s) this app belongs to. Used as a
    /// coarse cache-invalidation key: a "no achievements" cache entry
    /// stays valid only while the change number matches what was
    /// observed when the entry was recorded.
    pub change_number: u32,
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

    let mut summaries = Vec::new();

    for (app_id, change_number) in candidate_ids {
        // Type filter: keep only apps Steam reports as a game. ~11 µs per call
        // (verified against a 3 500-candidate library). Discards DLCs, tools,
        // Source SDK base, dedicated servers, soundtracks, demos, betas, and
        // entries with no cached type (typically depots / removed apps).
        // Case is not normalized by Steam — match case-insensitively.
        match client.app_type(app_id) {
            Some(t) if t.eq_ignore_ascii_case("game") => {}
            _ => continue,
        }

        // Release-state filter: skip pre-orders and preload-only entries.
        // Steam pipe rejects `connect(app_id)` for not-yet-released apps —
        // surfacing as a misleading "Steam client is not running" error in
        // the per-game worker. Apps with no `ReleaseState` key set are kept
        // (older games that pre-date the field still need to be scannable).
        if let Some(state) = client.get_app_data(app_id, c"ReleaseState")
            && !state.eq_ignore_ascii_case("released")
        {
            continue;
        }

        if apply_subscribed_filter && !client.is_subscribed_app(app_id) {
            continue;
        }

        // Name is left as a placeholder here. `app_name_for(id)` triggers
        // Steam's synchronous GetAppData fetch from server when local app
        // data is not cached, which for owned-but-never-launched games
        // blocks the pipe for tens of seconds per call. Across thousands of
        // candidates that explodes the probe latency. Names are filled in
        // later by the existing per-game subprocess worker which has the
        // pipe context to do it lazily on demand.
        summaries.push(GameSummary {
            app_id,
            name: format!("App {app_id}"),
            last_played: last_played_map.get(&app_id).copied(),
            achievement_count: 0,
            change_number,
        });
    }

    summaries.sort_by_key(|g| g.app_id);
    Ok(summaries)
}

/// Enumerate games the logged-in user has a license for.
///
/// This is a free-function entry point that delegates to `Client::enumerate_owned_games`.
/// Provided as a public API for use by callers that have a `Client` reference.
pub fn enumerate_owned_games(
    client: &Client,
    apply_subscribed_filter: bool,
) -> Result<Vec<GameSummary>, LibraryError> {
    enumerate_owned_games_impl(client, apply_subscribed_filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_summary_serde_roundtrip() {
        let summary = GameSummary {
            app_id: 12345,
            name: "Synthetic Game".to_owned(),
            last_played: Some(1_700_000_000),
            achievement_count: 0,
            change_number: 0,
        };

        let json = serde_json::to_string(&summary).expect("serialize");
        let restored: GameSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, summary);
    }

    #[test]
    fn game_summary_serde_no_last_played() {
        let summary = GameSummary {
            app_id: 99999,
            name: "Never Played".to_owned(),
            last_played: None,
            achievement_count: 7,
            change_number: 0,
        };

        let json = serde_json::to_string(&summary).expect("serialize");
        let restored: GameSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, summary);
    }
}
