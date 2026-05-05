use core::ffi::{c_char, c_void};
use core::marker::PhantomData;
use core::ptr::addr_of;
use std::ffi::CString;
use std::path::PathBuf;

use crate::error::{LibraryError, SteamError};
use crate::ffi::interfaces::{
    CallbackMessage, HSteamPipe, HSteamUser, ISteamApps001, ISteamApps008, ISteamClient018,
    ISteamFriends009, ISteamUser012, ISteamUtils005,
};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};
use crate::library::{GameSummary, enumerate_owned_games_impl};
use crate::stat_schema::{StatDescriptor, load as load_stat_descriptors};
use crate::steam_callback::{SteamCallback, callback_decode};
use crate::user_stats::UserStats;

const STEAM_CLIENT_VERSION: &str = "SteamClient018";
const STEAM_USER_VERSION: &str = "SteamUser012";
const STEAM_USER_STATS_VERSION: &str = "STEAMUSERSTATS_INTERFACE_VERSION013";
const STEAM_APPS_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION001";
const STEAM_APPS_008_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION008";
const STEAM_UTILS_VERSION: &str = "SteamUtils005";
const STEAM_FRIENDS_VERSION: &str = "SteamFriends009";

/// RGBA8888 pixels in row-major order; `rgba.len() == width * height * 4`.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct Client {
    steam_client: RawInterface,
    steam_user: RawInterface,
    steam_user_stats: RawInterface,
    steam_apps: RawInterface,
    steam_apps_008: RawInterface,
    steam_utils: RawInterface,
    steam_friends: RawInterface,
    pipe: HSteamPipe,
    user: HSteamUser,
    steam_id: u64,
    app_id: u32,
    _not_send: PhantomData<*const ()>,
}

impl Client {
    pub fn steam_id(&self) -> u64 {
        self.steam_id
    }

    pub fn app_id(&self) -> u32 {
        self.app_id
    }

    pub fn persona_name(&self) -> Option<String> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: live `SteamFriends009`.
        let raw_ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_persona_name)(self.steam_friends)
        };
        if raw_ptr.is_null() {
            return None;
        }
        // SAFETY: Steam returns a NUL-terminated UTF-8 string valid until the
        // next call on this pipe; we copy it before any further Steam call.
        let name = unsafe { std::ffi::CStr::from_ptr(raw_ptr) }
            .to_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)?;
        Some(name)
    }

    pub fn user_avatar(&self) -> Option<Image> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: live `SteamFriends009`. CSteamID is an 8-byte aggregate
        // passed as `u64` on SysV-x64.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_medium_friend_avatar)(self.steam_friends, self.steam_id)
        };
        if handle == 0 {
            return None;
        }
        self.get_image(handle).ok().flatten()
    }

    pub fn is_subscribed_app(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: live `ISteamApps008`.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_subscribed_app)(self.steam_apps_008, app_id)
        }
    }

    pub fn user_data_folder(&self) -> Result<PathBuf, SteamError> {
        let mut buf = [0u8; 1024];

        // SAFETY: live `ISteamUser012`; Steam writes at most `buf.len()`
        // bytes into the stack buffer.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUser012>(self.steam_user);
            ((*vtbl).get_user_data_folder)(
                self.steam_user,
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len() as i32,
            )
        };

        if !ok {
            return Err(SteamError::UserDataFolderUnavailable);
        }

        let nul_pos = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        if nul_pos == 0 {
            return Err(SteamError::UserDataFolderUnavailable);
        }

        let path_str = std::str::from_utf8(&buf[..nul_pos])
            .map_err(|_| SteamError::UserDataFolderUnavailable)?;

        Ok(PathBuf::from(path_str))
    }

    pub fn steam_root(&self) -> Result<PathBuf, SteamError> {
        let udf = self.user_data_folder()?;
        strip_userdata_suffix(udf)
    }

    pub fn app_name_for(&self, app_id: u32) -> Option<String> {
        if app_id == 0 || self.steam_apps.is_null() {
            return None;
        }

        let key = c"name";
        let mut buf = [0u8; 1024];

        // SAFETY: live `ISteamApps001`; `key` is static NUL-terminated;
        // Steam writes into `buf` and we copy out before any further call.
        let written = unsafe {
            let vtbl = opaque::vtable::<ISteamApps001>(self.steam_apps);
            ((*vtbl).get_app_data)(
                self.steam_apps,
                app_id,
                key.as_ptr(),
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len() as i32,
            )
        };

        if written <= 0 {
            return None;
        }

        let len = (written as usize).min(buf.len());
        let trimmed = buf[..len]
            .iter()
            .position(|&b| b == 0)
            .map_or(&buf[..len], |nul| &buf[..nul]);

        if trimmed.is_empty() {
            return None;
        }

        String::from_utf8(trimmed.to_vec()).ok()
    }

    pub fn app_name(&self) -> Option<String> {
        self.app_name_for(self.app_id)
    }

    pub fn is_app_installed(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: live `ISteamApps008`.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_app_installed)(self.steam_apps_008, app_id)
        }
    }

    pub fn app_type(&self, app_id: u32) -> Option<String> {
        self.get_app_data(app_id, c"type")
    }

    /// `None` when not cached locally; a subsequent call after Steam fires
    /// `AppDataChanged_t` may succeed once the daemon resolves the key.
    pub fn get_app_data(&self, app_id: u32, key: &core::ffi::CStr) -> Option<String> {
        if app_id == 0 || self.steam_apps.is_null() {
            return None;
        }
        let mut buf = [0u8; 1024];
        // SAFETY: same contract as `app_name_for`.
        let written = unsafe {
            let vtbl = opaque::vtable::<ISteamApps001>(self.steam_apps);
            ((*vtbl).get_app_data)(
                self.steam_apps,
                app_id,
                key.as_ptr(),
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len() as i32,
            )
        };
        if written <= 0 {
            return None;
        }
        let len = (written as usize).min(buf.len());
        let trimmed = buf[..len]
            .iter()
            .position(|&b| b == 0)
            .map_or(&buf[..len], |nul| &buf[..nul]);
        if trimmed.is_empty() {
            return None;
        }
        String::from_utf8(trimmed.to_vec()).ok()
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
        UserStats::from_raw(self.steam_user_stats)
    }

    /// `Ok(None)` for handle 0 — Steam is still fetching; retry once
    /// `AchievementIconFetched` (id 1408) fires.
    pub fn get_image(&self, handle: i32) -> Result<Option<Image>, SteamError> {
        if handle == 0 {
            return Ok(None);
        }
        if self.steam_utils.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_UTILS_VERSION.to_owned(),
            });
        }

        let mut width: u32 = 0;
        let mut height: u32 = 0;

        // SAFETY: live `ISteamUtils005`; Steam writes through the stack
        // pointers only when returning `true`.
        let size_ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils005>(self.steam_utils);
            ((*vtbl).get_image_size)(self.steam_utils, handle, &mut width, &mut height)
        };
        if !size_ok || width == 0 || height == 0 {
            return Ok(None);
        }

        let pixel_count = width as usize * height as usize;
        let byte_count = pixel_count * 4;
        let mut rgba: Vec<u8> = vec![0u8; byte_count];

        // SAFETY: `rgba` owns `byte_count` initialised bytes. Steam icons
        // top out around 256x256 (262144 B), so `byte_count as i32` cannot
        // overflow.
        let rgba_ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils005>(self.steam_utils);
            ((*vtbl).get_image_rgba)(
                self.steam_utils,
                handle,
                rgba.as_mut_ptr(),
                byte_count as i32,
            )
        };
        if !rgba_ok {
            return Ok(None);
        }

        Ok(Some(Image {
            width,
            height,
            rgba,
        }))
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
        if self.steam_utils.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: STEAM_UTILS_VERSION.to_owned(),
            });
        }

        let mut failed: bool = false;

        // SAFETY: live `ISteamUtils005`; Steam writes through the stack
        // `failed` pointer on completion-with-IO-error.
        let completed = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils005>(self.steam_utils);
            ((*vtbl).is_api_call_completed)(self.steam_utils, handle, &mut failed)
        };

        if !completed {
            return Ok(None);
        }

        if failed {
            return Ok(Some(Err(SteamError::CallFailed {
                method: "APICall(IO failure)",
            })));
        }

        let mut buf: Vec<u8> = vec![0u8; payload_size];

        // SAFETY: `buf` owns `payload_size` initialised bytes; observed
        // payloads top out at ~144 B, so the i32 cast is sound.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUtils005>(self.steam_utils);
            ((*vtbl).get_api_call_result)(
                self.steam_utils,
                handle,
                buf.as_mut_ptr().cast::<c_void>(),
                payload_size as i32,
                expected_callback_id,
                &mut failed,
            )
        };

        if !ok || failed {
            return Ok(Some(Err(SteamError::CallFailed {
                method: "GetAPICallResult",
            })));
        }

        Ok(Some(Ok(buf)))
    }

    /// Pure disk read of `appcache/stats/UserGameStatsSchema_<app_id>.bin`;
    /// `Ok(vec![])` when the file is missing (game never launched).
    pub fn stat_descriptors(&self, app_id: u32) -> Result<Vec<StatDescriptor>, SteamError> {
        load_stat_descriptors(app_id)
    }

    pub fn poll_callbacks(&self) -> Result<Vec<SteamCallback>, SteamError> {
        let library = loader::shared()?;
        let mut out = Vec::new();

        loop {
            let mut msg = CallbackMessage {
                user: 0,
                id: 0,
                param_ptr: core::ptr::null_mut(),
                param_size: 0,
            };

            // SAFETY: live pipe from `connect`; Steam writes through `msg`
            // only on `true`; null `call_handle` skips API-call tracking.
            let has_callback =
                library.b_get_callback(self.pipe, &mut msg, core::ptr::null_mut())?;
            if !has_callback {
                break;
            }

            // SAFETY: `msg` is `#[repr(packed)]`; taking a reference to a
            // packed field is UB, so we read each field unaligned.
            let id = unsafe { addr_of!(msg.id).read_unaligned() };
            let param_ptr = unsafe { addr_of!(msg.param_ptr).read_unaligned() };
            let param_size = unsafe { addr_of!(msg.param_size).read_unaligned() };

            let payload = if !param_ptr.is_null() && param_size > 0 {
                // SAFETY: Steam owns `param_ptr` for at least `param_size`
                // bytes until `free_last_callback`; we copy immediately.
                unsafe { core::slice::from_raw_parts(param_ptr, param_size as usize).to_vec() }
            } else {
                Vec::new()
            };

            library.free_last_callback(self.pipe)?;

            out.push(callback_decode::decode(crate::raw_callback::RawCallback {
                id,
                payload,
            }));
        }

        Ok(out)
    }
}

/// `app_id == 0` connects without an app context. A non-zero `app_id`
/// writes `SteamAppId` into the process environment — Steam reads it
/// exactly once during first-touch init, so call `connect` before
/// spawning any thread that reads `std::env`.
pub fn connect(app_id: u32) -> Result<Client, SteamError> {
    if app_id != 0 {
        // SAFETY: caller contract guarantees no other thread reads
        // `std::env` yet; the value is ASCII decimal.
        unsafe {
            std::env::set_var("SteamAppId", app_id.to_string());
        }
    }

    let library = loader::shared()?;
    let steam_client = library.create_interface(STEAM_CLIENT_VERSION)?;

    // SAFETY: `CreateInterface("SteamClient018")` guarantees the returned
    // object exposes an `ISteamClient018` vtable at offset 0.
    let pipe = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).create_steam_pipe)(steam_client)
    };
    if pipe == 0 {
        return Err(SteamError::SteamNotRunning);
    }

    // SAFETY: `pipe` is the freshly-vended live handle.
    let user = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).connect_to_global_user)(steam_client, pipe)
    };
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
    let steam_user = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_user)(steam_client, user, pipe, version.as_ptr())
    };
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
    let steam_user_stats = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_user_stats)(steam_client, user, pipe, stats_version.as_ptr())
    };
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
    let steam_apps = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_version.as_ptr())
    };

    let apps_008_version =
        CString::new(STEAM_APPS_008_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_APPS_008_VERSION.to_owned(),
        })?;

    // SAFETY: same as apps001 above; null is non-fatal.
    let steam_apps_008 = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_008_version.as_ptr())
    };

    let utils_version =
        CString::new(STEAM_UTILS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_UTILS_VERSION.to_owned(),
        })?;

    // SAFETY: `GetISteamUtils` takes (this, pipe, version) — no user
    // handle — per slot 9 of ISteamClient018. Null is non-fatal.
    let steam_utils = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_utils)(steam_client, pipe, utils_version.as_ptr())
    };

    let friends_version =
        CString::new(STEAM_FRIENDS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_FRIENDS_VERSION.to_owned(),
        })?;

    // SAFETY: live `user`/`pipe`; NUL-terminated version outlives the call.
    // Null is non-fatal — callsites null-guard.
    let steam_friends = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_friends)(steam_client, user, pipe, friends_version.as_ptr())
    };

    Ok(Client {
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

impl Drop for Client {
    fn drop(&mut self) {
        // SAFETY: handles were minted by this same `steam_client` in
        // `connect` on this thread (Client is `!Send`); sub-interface
        // pointers are owned by `steamclient.so` and not separately
        // released.
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

fn strip_userdata_suffix(path: PathBuf) -> Result<PathBuf, SteamError> {
    for ancestor in path.ancestors() {
        if ancestor.file_name().and_then(|s| s.to_str()) == Some("userdata")
            && let Some(parent) = ancestor.parent()
            && !parent.as_os_str().is_empty()
        {
            return Ok(parent.to_path_buf());
        }
    }
    Err(SteamError::MalformedUserDataPath { observed: path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_root_from_userdata_path_standard() {
        let udf = PathBuf::from("/home/x/.local/share/Steam/userdata/12345");
        let root = strip_userdata_suffix(udf).unwrap();
        assert_eq!(root, PathBuf::from("/home/x/.local/share/Steam"));
    }

    #[test]
    fn steam_root_from_real_steam_format_with_app_id_and_local() {
        let udf = PathBuf::from("/home/x/.local/share/Steam/userdata/12345/0/local");
        let root = strip_userdata_suffix(udf).unwrap();
        assert_eq!(root, PathBuf::from("/home/x/.local/share/Steam"));
    }

    #[test]
    fn steam_root_from_real_format_with_specific_app_id() {
        let udf = PathBuf::from("/opt/steam/userdata/9876/480/remote/cloud");
        let root = strip_userdata_suffix(udf).unwrap();
        assert_eq!(root, PathBuf::from("/opt/steam"));
    }

    #[test]
    fn steam_root_from_short_path() {
        let udf = PathBuf::from("/opt/steam/userdata/9876");
        let root = strip_userdata_suffix(udf).unwrap();
        assert_eq!(root, PathBuf::from("/opt/steam"));
    }

    #[test]
    fn steam_root_missing_userdata_component_returns_error() {
        let udf = PathBuf::from("/home/x/.local/share/Steam/12345");
        let err = strip_userdata_suffix(udf).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }

    #[test]
    fn steam_root_wrong_parent_name_returns_error() {
        let udf = PathBuf::from("/home/x/notsteam/notuserdata/12345");
        let err = strip_userdata_suffix(udf).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }

    #[test]
    fn steam_root_only_two_components_returns_error() {
        let udf = PathBuf::from("userdata/12345");
        let err = strip_userdata_suffix(udf).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }
}
