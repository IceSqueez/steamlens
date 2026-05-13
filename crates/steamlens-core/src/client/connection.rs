use core::marker::PhantomData;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::{
    HSteamPipe, HSteamUser, ISteamClient018, ISteamUser012, ISteamUser023,
};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};

pub(super) const STEAM_CLIENT_VERSION: &str = "SteamClient018";
pub(super) const STEAM_USER_VERSION: &str = "SteamUser012";
pub(super) const STEAM_USER_023_VERSION: &str = "SteamUser023";
pub(super) const STEAM_USER_STATS_VERSION: &str = "STEAMUSERSTATS_INTERFACE_VERSION013";
pub(super) const STEAM_APPS_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION001";
pub(super) const STEAM_APPS_008_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION008";
pub(super) const STEAM_UTILS_VERSION: &str = "SteamUtils005";
pub(super) const STEAM_FRIENDS_VERSION: &str = "SteamFriends009";

pub(super) struct SteamConnection {
    pub(super) steam_client: RawInterface,
    pub(super) steam_user: RawInterface,
    pub(super) steam_user_023: Option<RawInterface>,
    pub(super) steam_user_stats: RawInterface,
    pub(super) steam_apps: RawInterface,
    pub(super) steam_apps_008: RawInterface,
    pub(super) steam_utils: RawInterface,
    pub(super) steam_friends: RawInterface,
    pub(super) pipe: HSteamPipe,
    pub(super) user: HSteamUser,
    pub(super) steam_id: u64,
    pub(super) app_id: u32,
    pub(super) _not_send: PhantomData<*const ()>,
}

impl SteamConnection {
    pub(super) fn establish(app_id: u32) -> Result<Self, SteamError> {
        if app_id != 0 {
            // SAFETY: caller contract guarantees no other thread reads
            // `std::env` yet; the value is ASCII decimal.
            unsafe {
                std::env::set_var("SteamAppId", app_id.to_string());
            }
        }

        let library = loader::shared()?;

        tracing::info!(target: "establish", version = STEAM_CLIENT_VERSION, "create_interface: calling");
        let steam_client = library.create_interface(STEAM_CLIENT_VERSION)?;
        tracing::info!(target: "establish", ptr = ?steam_client, "create_interface: ok");

        // SAFETY: `CreateInterface("SteamClient018")` guarantees the returned
        // object exposes an `ISteamClient018` vtable at offset 0.
        tracing::info!(target: "establish", "create_steam_pipe: calling");
        let pipe = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).create_steam_pipe)(steam_client)
        };
        tracing::info!(target: "establish", pipe, "create_steam_pipe: returned");
        if pipe == 0 {
            return Err(SteamError::SteamNotRunning);
        }

        // SAFETY: `pipe` is the freshly-vended live handle.
        tracing::info!(target: "establish", "connect_to_global_user: calling");
        let user = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).connect_to_global_user)(steam_client, pipe)
        };
        tracing::info!(target: "establish", user, "connect_to_global_user: returned");
        if user == 0 {
            // SAFETY: release the pipe before bailing; otherwise IPC state
            // leaks in the steamclient process.
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::SteamNotRunning);
        }

        let version =
            CString::new(STEAM_USER_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
                version: STEAM_USER_VERSION.to_owned(),
            })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the
        // call. Returned vtable shape = `ISteamUser012`.
        tracing::info!(target: "establish", version = STEAM_USER_VERSION, "get_isteam_user(SteamUser012): calling");
        let steam_user = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_user)(steam_client, user, pipe, version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_user.is_null(), "get_isteam_user(SteamUser012): returned");
        if steam_user.is_null() {
            // SAFETY: release in reverse-init order.
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_user)(steam_client, pipe, user);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_USER_VERSION.to_owned(),
            });
        }

        // SAFETY: live `ISteamUser012`. On SysV-x64 the 8-byte CSteamID
        // aggregate is returned in RAX, so `-> u64` is correct.
        let steam_id = unsafe {
            let vtbl = opaque::vtable::<ISteamUser012>(steam_user);
            ((*vtbl).get_steam_id)(steam_user)
        };

        let stats_version = CString::new(STEAM_USER_STATS_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_USER_STATS_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        tracing::info!(target: "establish", version = STEAM_USER_STATS_VERSION, "get_isteam_user_stats: calling");
        let steam_user_stats = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_user_stats)(steam_client, user, pipe, stats_version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_user_stats.is_null(), "get_isteam_user_stats: returned");
        if steam_user_stats.is_null() {
            // SAFETY: release in reverse-init order.
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_user)(steam_client, pipe, user);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_USER_STATS_VERSION.to_owned(),
            });
        }

        let apps_version =
            CString::new(STEAM_APPS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
                version: STEAM_APPS_VERSION.to_owned(),
            })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        // Null return is non-fatal — callsites null-guard.
        tracing::info!(target: "establish", version = STEAM_APPS_VERSION, "get_isteam_apps(VERSION001): calling");
        let steam_apps = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_apps.is_null(), "get_isteam_apps(VERSION001): returned");

        let apps_008_version = CString::new(STEAM_APPS_008_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_APPS_008_VERSION.to_owned(),
            }
        })?;

        // SAFETY: same as apps001 above; null is non-fatal.
        tracing::info!(target: "establish", version = STEAM_APPS_008_VERSION, "get_isteam_apps(VERSION008): calling");
        let steam_apps_008 = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_008_version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_apps_008.is_null(), "get_isteam_apps(VERSION008): returned");

        let utils_version =
            CString::new(STEAM_UTILS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
                version: STEAM_UTILS_VERSION.to_owned(),
            })?;

        // SAFETY: `GetISteamUtils` takes (this, pipe, version) — no user
        // handle — per slot 9 of ISteamClient018. Null is non-fatal.
        tracing::info!(target: "establish", version = STEAM_UTILS_VERSION, "get_isteam_utils: calling");
        let steam_utils = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_utils)(steam_client, pipe, utils_version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_utils.is_null(), "get_isteam_utils: returned");

        let friends_version = CString::new(STEAM_FRIENDS_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_FRIENDS_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        // Null is non-fatal — callsites null-guard.
        tracing::info!(target: "establish", version = STEAM_FRIENDS_VERSION, "get_isteam_friends: calling");
        let steam_friends = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_friends)(steam_client, user, pipe, friends_version.as_ptr())
        };
        tracing::info!(target: "establish", null = steam_friends.is_null(), "get_isteam_friends: returned");

        let user_023_version = CString::new(STEAM_USER_023_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_USER_023_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        // Null is non-fatal — very old Steam clients may not expose SteamUser023;
        // stored as Option and null-guarded in the `User` sub-type.
        tracing::info!(target: "establish", version = STEAM_USER_023_VERSION, "get_isteam_user(SteamUser023): calling");
        let steam_user_023 = {
            let raw = unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).get_isteam_user)(steam_client, user, pipe, user_023_version.as_ptr())
            };
            tracing::info!(target: "establish", null = raw.is_null(), "get_isteam_user(SteamUser023): returned");
            if raw.is_null() { None } else { Some(raw) }
        };

        if let Some(u023) = steam_user_023 {
            // SAFETY:
            // - `u023` was returned by ISteamClient::GetISteamUser("SteamUser023") above,
            //   using valid `pipe` and `user` handles produced by connect_to_global_user
            //   (both checked non-zero before this point).
            // - vtable slot 1 (`b_logged_on`) verified against Steamworks.NET isteamuser.h
            //   (see STEAM_NOTES.md → Vtable Offset Verifications).
            // - Called on the same thread that created the pipe; SteamConnection is `!Send`
            //   via `PhantomData<*const ()>`. SysV-x64 ABI: `this` in RDI, bool in AL.
            tracing::info!(target: "establish", "b_logged_on: calling");
            let logged_on = unsafe {
                let vtbl = opaque::vtable::<ISteamUser023>(u023);
                ((*vtbl).b_logged_on)(u023)
            };
            tracing::info!(target: "establish", logged_on, "b_logged_on: returned");
            if !logged_on {
                unsafe {
                    let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                    ((*vtbl).release_user)(steam_client, pipe, user);
                    ((*vtbl).release_steam_pipe)(steam_client, pipe);
                }
                return Err(SteamError::NotLoggedIn);
            }
        }

        Ok(SteamConnection {
            steam_client,
            steam_user,
            steam_user_023,
            steam_user_stats,
            steam_apps,
            steam_apps_008,
            steam_utils,
            steam_friends,
            pipe,
            user,
            steam_id,
            app_id,
            _not_send: PhantomData,
        })
    }
}

impl Drop for SteamConnection {
    fn drop(&mut self) {
        // SAFETY: handles were minted by this same `steam_client` in
        // `establish` on this thread (SteamConnection is `!Send` via
        // `PhantomData<*const ()>`); sub-interface pointers are owned by
        // `steamclient.so` and released transitively when `user` and `pipe`
        // are released — no per-interface release API exists on ISteamClient018
        // (confirmed: Steamworks.NET isteamclient.h exposes only
        // `BReleaseSteamPipe` and `ReleaseUser`).
        unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(self.steam_client);
            if self.user != 0 {
                ((*vtbl).release_user)(self.steam_client, self.pipe, self.user);
            }
            if self.pipe != 0 {
                ((*vtbl).release_steam_pipe)(self.steam_client, self.pipe);
            }
        }
    }
}
