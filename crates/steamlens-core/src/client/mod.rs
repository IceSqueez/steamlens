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

    pub fn get_player_steam_level(&self) -> Option<u32> {
        self.user.get_player_steam_level()
    }

    pub fn nickname(&self) -> Option<String> {
        self.friends.nickname()
    }

    pub fn user_avatar(&self) -> Option<Image> {
        self.friends
            .user_avatar(|handle| self.get_image(handle).ok().flatten())
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

    pub fn get_app_data(&self, app_id: u32, key: &core::ffi::CStr) -> Option<String> {
        self.apps.get_app_data(app_id, key)
    }

    pub fn enumerate_owned_games(
        &self,
        apply_subscribed_filter: bool,
    ) -> Result<Vec<GameSummary>, LibraryError> {
        enumerate_owned_games_impl(self, apply_subscribed_filter)
    }

    pub fn user_stats(&self) -> UserStats<'_> {
        UserStats::from_raw(self.conn.steam_user_stats)
    }

    pub fn get_image(&self, handle: i32) -> Result<Option<Image>, SteamError> {
        self.utils.get_image(handle)
    }

    pub fn poll_call_result(
        &self,
        handle: u64,
        expected_callback_id: i32,
        payload_size: usize,
    ) -> Result<Option<Result<Vec<u8>, SteamError>>, SteamError> {
        self.utils
            .poll_call_result(handle, expected_callback_id, payload_size)
    }

    pub fn stat_descriptors(&self, app_id: u32) -> Result<Vec<StatDescriptor>, SteamError> {
        load_stat_descriptors(app_id)
    }

    pub fn poll_callbacks(&self) -> Result<Vec<SteamCallback>, SteamError> {
        self.callbacks.poll_callbacks()
    }
}

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
