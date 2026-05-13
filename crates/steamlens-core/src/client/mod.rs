mod apps;
mod callbacks;
mod connection;
mod friends;
mod internal;
mod user;
mod utils;

use crate::error::{LibraryError, SteamError};
use crate::library::{GameSummary, enumerate_owned_games_impl};
use crate::stat_schema::{StatDescriptor, load as load_stat_descriptors};
use crate::steam_callback::SteamCallback;
use crate::user_stats::UserStats;

pub use utils::Image;

use apps::Apps;
use callbacks::Callbacks;
use connection::SteamConnection;
use friends::Friends;
use user::User;
use utils::Utils;

pub struct Client {
    conn: SteamConnection,
    apps: Apps,
    friends: Friends,
    utils: Utils,
    callbacks: Callbacks,
    user: User,
}

impl Client {
    pub fn steam_id(&self) -> u64 {
        self.user.steam_id()
    }

    pub fn app_id(&self) -> u32 {
        self.user.app_id()
    }

    /// Returns `None` when `SteamUser023` is unavailable (very old Steam
    /// client) or when Steam returns a negative value (not a valid level).
    pub fn get_player_steam_level(&self) -> Option<u32> {
        self.user.get_player_steam_level()
    }

    pub fn persona_name(&self) -> Option<String> {
        self.friends.persona_name()
    }

    /// Reads the avatar PNG that Steam already cached on disk at
    /// `<steam_root>/config/avatarcache/<steamid64>.png`. Bypasses
    /// `ISteamFriends::GetMediumFriendAvatar` + `ISteamUtils::GetImageRGBA`
    /// entirely so we don't depend on the Steam image-handle pipeline.
    /// Returns `None` if the file is missing or undecodable — the avatar
    /// is non-essential, no error needs to bubble.
    pub fn user_avatar(&self) -> Option<Image> {
        crate::avatar::read_local_avatar(self.steam_id()).ok()
    }

    pub fn is_subscribed_app(&self, app_id: u32) -> bool {
        self.apps.is_subscribed_app(app_id)
    }

    pub fn user_data_folder(&self) -> Result<std::path::PathBuf, SteamError> {
        self.user.user_data_folder()
    }

    pub fn steam_root(&self) -> Result<std::path::PathBuf, SteamError> {
        self.user.steam_root()
    }

    pub fn app_name(&self) -> Option<String> {
        self.apps.app_name()
    }

    pub fn is_app_installed(&self, app_id: u32) -> bool {
        self.apps.is_app_installed(app_id)
    }

    pub fn app_type(&self, app_id: u32) -> Option<String> {
        self.apps.app_type(app_id)
    }

    /// `None` when not cached locally; a subsequent call after Steam fires
    /// `AppDataChanged_t` may succeed once the daemon resolves the key.
    pub fn get_app_data(&self, app_id: u32, key: &core::ffi::CStr) -> Option<String> {
        self.apps.get_app_data(app_id, key)
    }

    pub fn enumerate_owned_games(
        &self,
        apply_subscribed_filter: bool,
    ) -> Result<Vec<GameSummary>, LibraryError> {
        enumerate_owned_games_impl(self, apply_subscribed_filter)
    }

    /// Getters return Steam defaults (0 / `false`) until `RequestUserStats`
    /// completes; setters stage locally and require `store_stats` to persist.
    pub fn user_stats(&self) -> UserStats<'_> {
        UserStats::from_raw(self.conn.steam_user_stats)
    }

    /// `Ok(None)` for handle 0 — Steam is still fetching; retry once
    /// `AchievementIconFetched` (id 1408) fires.
    pub fn get_image(&self, handle: i32) -> Result<Option<Image>, SteamError> {
        self.utils.get_image(handle)
    }

    /// Per-call async result bound to a `SteamAPICall_t`; these do NOT
    /// appear in the broadcast queue drained by [`Self::poll_callbacks`].
    /// Returns `None` while pending — caller retries ~50 ms later.
    pub fn poll_call_result(
        &self,
        handle: u64,
        expected_callback_id: i32,
        payload_size: usize,
    ) -> Result<Option<Result<Vec<u8>, SteamError>>, SteamError> {
        self.utils
            .poll_call_result(handle, expected_callback_id, payload_size)
    }

    /// Pure disk read of `appcache/stats/UserGameStatsSchema_<app_id>.bin`;
    /// `Ok(vec![])` when the file is missing (game never launched).
    pub fn stat_descriptors(&self, app_id: u32) -> Result<Vec<StatDescriptor>, SteamError> {
        load_stat_descriptors(app_id)
    }

    pub fn poll_callbacks(&self) -> Result<Vec<SteamCallback>, SteamError> {
        self.callbacks.poll_callbacks()
    }
}

/// `app_id == 0` connects without an app context. A non-zero `app_id`
/// writes `SteamAppId` into the process environment — Steam reads it
/// exactly once during first-touch init, so call `connect` before
/// spawning any thread that reads `std::env`.
pub fn connect(app_id: u32) -> Result<Client, SteamError> {
    let conn = SteamConnection::establish(app_id)?;

    let apps = Apps {
        steam_apps: conn.steam_apps,
        steam_apps_008: conn.steam_apps_008,
        app_id: conn.app_id,
    };
    let friends = Friends {
        steam_friends: conn.steam_friends,
        steam_id: conn.steam_id,
    };
    let utils = Utils {
        steam_utils: conn.steam_utils,
    };
    let callbacks = Callbacks { pipe: conn.pipe };
    let user = User {
        steam_user: conn.steam_user,
        steam_user_023: conn.steam_user_023,
        steam_id: conn.steam_id,
        app_id: conn.app_id,
    };

    Ok(Client {
        conn,
        apps,
        friends,
        utils,
        callbacks,
        user,
    })
}
