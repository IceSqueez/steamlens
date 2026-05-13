use core::marker::PhantomData;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::{HSteamPipe, HSteamUser, ISteamClient018, ISteamUser023};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};

pub(super) const STEAM_CLIENT_VERSION: &str = "SteamClient018";
pub(super) const STEAM_USER_023_VERSION: &str = "SteamUser023";
pub(super) const STEAM_USER_STATS_VERSION: &str = "STEAMUSERSTATS_INTERFACE_VERSION013";
pub(super) const STEAM_APPS_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION001";
pub(super) const STEAM_APPS_008_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION008";
pub(super) const STEAM_UTILS_VERSION: &str = "SteamUtils010";
pub(super) const STEAM_FRIENDS_VERSION: &str = "SteamFriends009";

pub(super) struct SteamConnection {
    pub(super) steam_client: RawInterface,
    pub(super) steam_user: RawInterface,
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

        tracing::trace!("establish: loader::shared() pre");
        let library = loader::shared()?;

        tracing::trace!(
            version = STEAM_CLIENT_VERSION,
            "establish: create_interface pre"
        );
        let steam_client = library.create_interface(STEAM_CLIENT_VERSION)?;
        tracing::trace!(
            version = STEAM_CLIENT_VERSION,
            "establish: create_interface post"
        );

        // SAFETY: `CreateInterface("SteamClient018")` guarantees the returned
        // object exposes an `ISteamClient018` vtable at offset 0.
        tracing::trace!("establish: create_steam_pipe pre");
        let pipe = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).create_steam_pipe)(steam_client)
        };
        tracing::trace!(pipe, "establish: create_steam_pipe post");
        if pipe == 0 {
            return Err(SteamError::SteamNotRunning);
        }

        // SAFETY: `pipe` is the freshly-vended live handle.
        tracing::trace!("establish: connect_to_global_user pre");
        let user = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).connect_to_global_user)(steam_client, pipe)
        };
        tracing::trace!(user, "establish: connect_to_global_user post");
        if user == 0 {
            // SAFETY: release the pipe before bailing; otherwise IPC state
            // leaks in the steamclient process.
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::SteamNotRunning);
        }

        let user_023_version = CString::new(STEAM_USER_023_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_USER_023_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the
        // call. Returned vtable shape = `ISteamUser023`.
        tracing::trace!(
            version = STEAM_USER_023_VERSION,
            "establish: get_isteam_user pre"
        );
        let steam_user = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_user)(steam_client, user, pipe, user_023_version.as_ptr())
        };
        tracing::trace!(
            null = steam_user.is_null(),
            "establish: get_isteam_user post"
        );
        if steam_user.is_null() {
            // SAFETY: release in reverse-init order.
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_user)(steam_client, pipe, user);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_USER_023_VERSION.to_owned(),
            });
        }

        // SAFETY: live `ISteamUser023`, slot 1 = `BLoggedOn`; called on the
        // `!Send` owner thread; primitive `bool` return.
        tracing::trace!("establish: b_logged_on pre");
        let logged_on = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(steam_user);
            ((*vtbl).b_logged_on)(steam_user)
        };
        tracing::trace!(logged_on, "establish: b_logged_on post");
        if !logged_on {
            unsafe {
                let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
                ((*vtbl).release_user)(steam_client, pipe, user);
                ((*vtbl).release_steam_pipe)(steam_client, pipe);
            }
            return Err(SteamError::NotLoggedIn);
        }

        // SAFETY: live `ISteamUser023`, slot 2 = `GetSteamID`. CSteamID has a
        // user-defined ctor → MSVC x64 returns via hidden sret out-pointer;
        // SysV x64 returns inline in RAX. cfg-gated below to match each ABI.
        tracing::trace!("establish: get_steam_id pre");
        #[cfg(target_os = "windows")]
        let steam_id: u64 = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(steam_user);
            let mut out: u64 = 0;
            let _ = ((*vtbl).get_steam_id)(steam_user, &mut out);
            out
        };
        #[cfg(not(target_os = "windows"))]
        let steam_id: u64 = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(steam_user);
            ((*vtbl).get_steam_id)(steam_user)
        };
        tracing::trace!(steam_id, "establish: get_steam_id post");

        let stats_version = CString::new(STEAM_USER_STATS_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_USER_STATS_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        tracing::trace!(
            version = STEAM_USER_STATS_VERSION,
            "establish: get_isteam_user_stats pre"
        );
        let steam_user_stats = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_user_stats)(steam_client, user, pipe, stats_version.as_ptr())
        };
        tracing::trace!(
            null = steam_user_stats.is_null(),
            "establish: get_isteam_user_stats post"
        );
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
        tracing::trace!(
            version = STEAM_APPS_VERSION,
            "establish: get_isteam_apps(001) pre"
        );
        let steam_apps = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_version.as_ptr())
        };
        tracing::trace!(
            null = steam_apps.is_null(),
            "establish: get_isteam_apps(001) post"
        );

        let apps_008_version = CString::new(STEAM_APPS_008_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_APPS_008_VERSION.to_owned(),
            }
        })?;

        // SAFETY: same as apps001 above; null is non-fatal.
        tracing::trace!(
            version = STEAM_APPS_008_VERSION,
            "establish: get_isteam_apps(008) pre"
        );
        let steam_apps_008 = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_008_version.as_ptr())
        };
        tracing::trace!(
            null = steam_apps_008.is_null(),
            "establish: get_isteam_apps(008) post"
        );

        let utils_version =
            CString::new(STEAM_UTILS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
                version: STEAM_UTILS_VERSION.to_owned(),
            })?;

        // SAFETY: `GetISteamUtils` takes (this, pipe, version) — no user
        // handle — per slot 9 of ISteamClient018. Null is non-fatal.
        tracing::trace!(
            version = STEAM_UTILS_VERSION,
            "establish: get_isteam_utils pre"
        );
        let steam_utils = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_utils)(steam_client, pipe, utils_version.as_ptr())
        };
        tracing::trace!(
            null = steam_utils.is_null(),
            "establish: get_isteam_utils post"
        );

        let friends_version = CString::new(STEAM_FRIENDS_VERSION).map_err(|_| {
            SteamError::InvalidInterfaceVersion {
                version: STEAM_FRIENDS_VERSION.to_owned(),
            }
        })?;

        // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
        // Null is non-fatal — callsites null-guard.
        tracing::trace!(
            version = STEAM_FRIENDS_VERSION,
            "establish: get_isteam_friends pre"
        );
        let steam_friends = unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).get_isteam_friends)(steam_client, user, pipe, friends_version.as_ptr())
        };
        tracing::trace!(
            null = steam_friends.is_null(),
            "establish: get_isteam_friends post"
        );

        Ok(SteamConnection {
            steam_client,
            steam_user,
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
        // SAFETY: handles were minted by this `steam_client` in `establish` on
        // the `!Send` owner thread; sub-interface pointers are owned by
        // `steamclient` and released transitively when `user` and `pipe`
        // close — no per-interface release API exists on `ISteamClient018`.
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
