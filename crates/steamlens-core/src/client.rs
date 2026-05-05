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

/// RGBA8888 pixel data for a Steam image handle.
///
/// `rgba` has exactly `width * height * 4` bytes. Pixels are in
/// left-to-right, top-to-bottom order with no row padding.
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

    /// Returns the logged-in user's current persona name (display name on Steam).
    ///
    /// Copies the NUL-terminated string Steam writes into its own memory into an
    /// owned `String` before returning. The raw pointer from Steam is valid only
    /// until the next Steam call on this pipe — it is never stored.
    ///
    /// Returns `None` when the `ISteamFriends009` interface pointer is null or
    /// Steam returns an empty name.
    pub fn persona_name(&self) -> Option<String> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: `self.steam_friends` was vended as "SteamFriends009" so its
        // vtable layout matches `ISteamFriends009`. `GetPersonaName` (slot 0)
        // returns a pointer to Steam-owned memory that is valid until the next
        // Steam call on this pipe. We copy it into an owned `String` immediately
        // before any further Steam call can occur, so the pointer is never
        // dangling when used. No pointer into Steam-owned memory is stored.
        let raw_ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_persona_name)(self.steam_friends)
        };
        if raw_ptr.is_null() {
            return None;
        }
        // SAFETY: Steam guarantees `GetPersonaName` returns a NUL-terminated
        // UTF-8 string (documented in the SDK as "stored in UTF-8 format").
        // The pointer is valid until the next Steam call — we copy it here
        // before any other call takes place.
        let name = unsafe { std::ffi::CStr::from_ptr(raw_ptr) }
            .to_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)?;
        Some(name)
    }

    /// Returns the medium (64×64) avatar image for the logged-in user.
    ///
    /// Calls `GetMediumFriendAvatar(steam_id)` to get an image handle, then
    /// resolves the RGBA8888 pixel data via `get_image`. Returns `None` when
    /// the handle is 0 (avatar not yet loaded by Steam) or the interface is
    /// unavailable.
    pub fn user_avatar(&self) -> Option<Image> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: `self.steam_friends` was vended as "SteamFriends009" so its
        // vtable layout matches `ISteamFriends009`. `GetMediumFriendAvatar`
        // (slot 26) takes a `CSteamID` passed as a u64 on SysV-x64 (the
        // 8-byte aggregate is passed in a register, same as `GetSteamID`
        // returns in RAX). Returns an image handle i32; handle 0 = not loaded.
        // No pointer is retained after this call.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_medium_friend_avatar)(self.steam_friends, self.steam_id)
        };
        if handle == 0 {
            return None;
        }
        self.get_image(handle).ok().flatten()
    }

    /// Returns the human-readable display name for the connected app, or `None`
    /// when `app_id` is 0, Steam has no entry for the app, or the returned
    /// buffer is empty.
    ///
    /// Data is served from Steam's local appcache — no network request is made.
    /// The returned string is owned and heap-allocated; no pointer into Steam's
    /// memory is retained after this call returns.
    /// Returns `true` if the active user has a current license for `app_id`,
    /// per `ISteamApps008::BIsSubscribedApp`. Returns `false` for refunded,
    /// expired, or never-owned apps. Returns `false` if `apps008` is null
    /// (interface unavailable on this Steam build).
    pub fn is_subscribed_app(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: `self.steam_apps_008` was vended as
        // "STEAMAPPS_INTERFACE_VERSION008" so its vtable layout matches
        // `ISteamApps008`. Slot 6 is `is_subscribed_app(this, app_id) -> bool`
        // per the canonical interface definition. No pointer to Steam-owned
        // memory is retained.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_subscribed_app)(self.steam_apps_008, app_id)
        }
    }

    /// Returns the path to the current user's Steam data folder.
    ///
    /// Calls `ISteamUser012::GetUserDataFolder` (vtable slot 6), which writes
    /// a NUL-terminated path such as `/home/x/.local/share/Steam/userdata/12345`
    /// into a caller-owned stack buffer. The result is copied into a `PathBuf`
    /// before returning.
    ///
    /// # Errors
    ///
    /// [`SteamError::UserDataFolderUnavailable`] when the call returns `false`
    /// or the returned path is empty.
    pub fn user_data_folder(&self) -> Result<PathBuf, SteamError> {
        let mut buf = [0u8; 1024];

        // SAFETY: `self.steam_user` was vended as "SteamUser012" so its vtable
        // layout matches `ISteamUser012`. Slot 6 is `GetUserDataFolder(this,
        // buffer, buffer_size) -> bool` per the canonical interface definition.
        // `buf.as_mut_ptr()` points to 1024 bytes of stack-allocated storage
        // that lives for the duration of this call. Steam writes at most
        // `buffer_size` bytes (1024) into the buffer. The NUL-terminated bytes
        // are read into an owned `PathBuf` immediately after the call before
        // any subsequent Steam call. No pointer into Steam-owned memory is
        // retained after this call returns.
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

    /// Returns the Steam installation root by deriving it from `user_data_folder`.
    ///
    /// `GetUserDataFolder` returns `<steam_root>/userdata/<steamid3>`.
    /// This method strips the last two path components (`<steamid3>` and
    /// `userdata`) to produce `<steam_root>`.
    ///
    /// # Errors
    ///
    /// [`SteamError::UserDataFolderUnavailable`] when `user_data_folder` fails.
    /// [`SteamError::MalformedUserDataPath`] when the returned path does not
    /// end with a `userdata/<steamid3>` component pair.
    pub fn steam_root(&self) -> Result<PathBuf, SteamError> {
        let udf = self.user_data_folder()?;
        strip_userdata_suffix(udf)
    }

    /// Returns the human-readable display name for the given `app_id`, or
    /// `None` when Steam has no entry for the app or the returned buffer is
    /// empty.
    ///
    /// Data is served from Steam's local appcache — no network request is made.
    /// The returned string is owned; no pointer into Steam memory is retained.
    pub fn app_name_for(&self, app_id: u32) -> Option<String> {
        if app_id == 0 || self.steam_apps.is_null() {
            return None;
        }

        let key = c"name";
        let mut buf = [0u8; 1024];

        // SAFETY: `self.steam_apps` was vended as "STEAMAPPS_INTERFACE_VERSION001"
        // so its vtable layout matches `ISteamApps001`. Slot 0 is
        // `GetAppData(this, app_id, key, value, value_length) -> i32`.
        // `key.as_ptr()` is a static NUL-terminated C string. `buf.as_mut_ptr()`
        // is a valid, aligned, uniquely-owned buffer of `buf.len()` bytes.
        // Steam writes at most `value_length` bytes into it. The written bytes
        // are copied into an owned `String` immediately after the call. No
        // pointer into Steam-owned memory is retained.
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

    /// Returns `true` if the app's content is currently installed on disk
    /// (ISteamApps008 slot 19 `BIsAppInstalled`). False for owned-but-not-
    /// downloaded games. Distinguishing this matters because connecting a
    /// Steam pipe with an app_id whose content is not installed makes Steam
    /// reject the bind (observed: `connect(app_id)` returns generic failure
    /// or "SteamNotRunning" for these app_ids).
    pub fn is_app_installed(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: steam_apps_008 vended as ISteamApps008. Slot 19 is
        // `BIsAppInstalled(this, app_id) -> bool`.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_app_installed)(self.steam_apps_008, app_id)
        }
    }

    /// Returns the app's type as reported by Steam (`"Game"`, `"DLC"`,
    /// `"Tool"`, `"Music"`, `"Config"`, `"Beta"`, `"Demo"`, `"Application"`,
    /// `"Music"`, `"Video"`, `"Mod"`, `"Hardware"`, `"Series"`, etc.).
    /// Case is not normalized by Steam — callers must compare
    /// case-insensitively. Returns `None` when the type is not cached
    /// locally (rare; observed ~6 % of a typical packageinfo).
    pub fn app_type(&self, app_id: u32) -> Option<String> {
        self.get_app_data(app_id, c"type")
    }

    /// Generic accessor for Steam's per-app metadata via `ISteamApps001::GetAppData`.
    ///
    /// `key` must be a NUL-terminated C string. Common keys: `c"name"`,
    /// `c"type"`, `c"header_image"`, `c"oslist"`. Returns `None` when the key
    /// has no value cached locally; Steam may begin a background fetch and
    /// the next call after `AppDataChanged_t` fires can return a value.
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

    /// Enumerate all games the logged-in user has a license for, returning a
    /// sorted `Vec<GameSummary>`.
    ///
    /// When `apply_subscribed_filter` is `true`, each candidate app_id from
    /// `packageinfo.vdf` is verified via `BIsSubscribedApp` before inclusion.
    /// When `false`, all app_ids from `packageinfo.vdf` that return a non-empty
    /// name from `GetAppData` are included — useful for testing the raw
    /// candidate set before enabling the ownership filter.
    ///
    /// # Errors
    ///
    /// [`LibraryError::SteamRoot`] — could not derive the Steam root from the pipe.
    /// [`LibraryError::PackageInfoIo`] — `packageinfo.vdf` could not be read.
    /// [`LibraryError::PackageInfoParse`] — `packageinfo.vdf` is malformed.
    pub fn enumerate_owned_games(
        &self,
        apply_subscribed_filter: bool,
    ) -> Result<Vec<GameSummary>, LibraryError> {
        enumerate_owned_games_impl(self, apply_subscribed_filter)
    }

    /// Returns the user-stats sub-interface for achievement and stat operations.
    ///
    /// Until `RequestUserStats` completes (Round 2), `get_*` methods return
    /// Steam's defaults (typically 0 / `false`). `set_*` calls stage changes
    /// locally; `store_stats` must be called to persist them.
    pub fn user_stats(&self) -> UserStats<'_> {
        UserStats::from_raw(self.steam_user_stats)
    }

    /// Fetch RGBA pixel data for a Steam image handle.
    ///
    /// Handles are obtained from [`UserStats::achievement_icon`], which returns
    /// an `i32` for each achievement name. A handle of `0` means Steam has not
    /// finished fetching the image yet — this is the normal state immediately
    /// after `RequestUserStats` returns and before Steam has retrieved the icon
    /// data asynchronously. In that case `Ok(None)` is returned and the caller
    /// should retry on the next callback poll cycle.
    ///
    /// When Steam emits an `AchievementIconFetched` callback (id 1408) the
    /// handle becomes valid and this method will return the pixel data.
    ///
    /// Returns `Ok(None)` when the handle is `0`, when Steam reports the handle
    /// is not loaded yet, or if a race causes `GetImageRGBA` to fail.
    ///
    /// # Errors
    ///
    /// [`SteamError::InterfaceUnavailable`] when the `SteamUtils005` interface
    /// pointer is null (should not happen in normal usage after `connect`).
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

        // SAFETY: `self.steam_utils` was vended as "SteamUtils005", so its
        // vtable layout matches `ISteamUtils005`. `handle` is an opaque image
        // handle from Steam — we pass it back unchanged. `width` and `height`
        // are stack-allocated `u32`s whose addresses are valid for the duration
        // of this call. Steam writes into them when it returns `true`. No
        // pointer into Steam-owned memory is retained after this call.
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

        // SAFETY: `self.steam_utils` vtable invariant: same as above for
        // `GetImageSize`. `rgba.as_mut_ptr()` points to the start of a
        // Vec<u8> with `byte_count` bytes of allocated, initialised storage.
        // Steam writes exactly `dest_size` bytes (RGBA8888). `byte_count`
        // fits in an `i32` for any practical image (Steam icon max ~256×256
        // = 262144 bytes, well below i32::MAX). No pointer is retained after
        // this call — the Vec owns the data.
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

    /// Poll the completion status of a Steam API call result.
    ///
    /// Steam delivers some async results (e.g. `RequestGlobalAchievementPercentages`)
    /// as *Call Results* rather than broadcast callbacks. Call Results are bound to the
    /// specific `SteamAPICall_t` handle returned by the initiating method. They do NOT
    /// appear in the broadcast queue polled by [`Self::poll_callbacks`] — they must be
    /// retrieved via `ISteamUtils::IsAPICallCompleted` + `GetAPICallResult`.
    ///
    /// Returns:
    /// - `None` — the call is still pending; poll again later (suggested: 50 ms).
    /// - `Some(Ok(bytes))` — completed successfully; `bytes` contains the callback
    ///   payload of `payload_size` bytes (caller interprets the layout).
    /// - `Some(Err(e))` — completed with an IO-level failure.
    ///
    /// `expected_callback_id` must match the callback ID of the expected result type
    /// (e.g. `1110` for `GlobalAchievementPercentagesReady_t`). Steam uses it for
    /// type-checking on the C++ side.
    ///
    /// # Errors
    ///
    /// [`SteamError::InterfaceUnavailable`] when `SteamUtils005` is null (should
    /// not happen after a successful `connect`).
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

        // SAFETY: `self.steam_utils` was vended as "SteamUtils005" so its vtable layout
        // matches `ISteamUtils005`. `handle` is the SteamAPICall_t returned by the
        // initiating method. `failed` is a stack bool whose address is valid for the
        // duration of the call. Steam writes `true` into it when the call completed with
        // an IO error. Returns `false` when the call is still pending.
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

        // SAFETY: `self.steam_utils` vtable invariant: same as IsAPICallCompleted above.
        // `buf.as_mut_ptr()` points to `payload_size` bytes of initialised storage.
        // `callback_size` = payload_size as i32, fits in i32 for any reasonable callback
        // payload (max observed: 144 bytes for UserAchievementIconFetched).
        // `expected_callback_id` is the Steam callback ID for type-checking.
        // `failed` is a fresh stack bool. Steam writes the callback bytes into `buf`
        // when it returns `true`. No pointer is retained after this call.
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

    /// Read stat counter metadata from the local Steam schema cache file.
    ///
    /// Returns a descriptor for every integer and float stat defined for `app_id`.
    /// Achievement-typed entries in the schema are filtered out — this method
    /// returns only pure stat counters (those with `type = "INT"` or `"FLOAT"`).
    ///
    /// Reads `~/.local/share/Steam/appcache/stats/UserGameStatsSchema_<app_id>.bin`
    /// from disk; no live Steam connection is required.
    ///
    /// # Missing file
    ///
    /// If the schema file does not exist (the game was never launched against the
    /// connected Steam account, or stats have not been downloaded yet) the method
    /// returns `Ok(Vec::new())` — an empty list is not an error.
    ///
    /// # Errors
    ///
    /// [`SteamError::SchemaParseError`] when the cache file is present but the
    /// binary KeyValue data is truncated or otherwise corrupt.
    pub fn stat_descriptors(&self, app_id: u32) -> Result<Vec<StatDescriptor>, SteamError> {
        load_stat_descriptors(app_id)
    }

    /// Drain all pending Steam callbacks and return them as typed [`SteamCallback`] values.
    ///
    /// Each call processes every callback that Steam has queued since the previous poll.
    /// Callbacks are returned in arrival order. Known callback IDs are decoded into typed
    /// variants; unrecognised or malformed payloads become `SteamCallback::Unknown` with
    /// the raw id and payload bytes preserved.
    ///
    /// The method is intended to be called at ~10 Hz from an `iced::Subscription`
    /// tick on the UI thread. It is not safe to call from multiple threads
    /// concurrently — `Client` is `!Send`.
    ///
    /// Returns an empty `Vec` when no callbacks are pending, which is the
    /// normal case between Steam events. Never returns an error for an empty
    /// queue; a `Result` error indicates an FFI symbol resolution failure which
    /// is unrecoverable on this pipe.
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

            // SAFETY: `msg` is a stack-allocated `CallbackMessage` whose
            // address is valid for the duration of this call. `self.pipe`
            // is the live `HSteamPipe` handle established in `connect()`.
            // Steam writes into `msg` only when it returns `true` (a callback
            // is available). We pass `null_mut()` for the call-handle output
            // because we do not use API-call tracking at this layer.
            let has_callback =
                library.b_get_callback(self.pipe, &mut msg, core::ptr::null_mut())?;
            if !has_callback {
                break;
            }

            // Read fields from the packed struct via `addr_of!` to avoid
            // creating unaligned references, which is UB even for reads on
            // packed structs. Steam wrote valid data when `has_callback` is
            // true.
            //
            // SAFETY: `msg` was just written by Steam (b_get_callback returned
            // true). `param_ptr` and `param_size` are valid for the duration
            // between b_get_callback and free_last_callback — we copy the
            // bytes immediately before calling free.
            let id = unsafe { addr_of!(msg.id).read_unaligned() };
            let param_ptr = unsafe { addr_of!(msg.param_ptr).read_unaligned() };
            let param_size = unsafe { addr_of!(msg.param_size).read_unaligned() };

            let payload = if !param_ptr.is_null() && param_size > 0 {
                // SAFETY: Steam guarantees that `param_ptr` points to a buffer
                // of at least `param_size` bytes that is valid until
                // `Steam_FreeLastCallback` is called on this pipe. We copy the
                // bytes into an owned `Vec` before freeing. The cast from i32
                // to usize is lossless for positive i32 on x86_64. We trust
                // Steam to report `param_size` accurately for its own
                // allocation — this is the standard FFI trust boundary.
                unsafe { core::slice::from_raw_parts(param_ptr, param_size as usize).to_vec() }
            } else {
                Vec::new()
            };

            // Free the callback buffer before moving to the next iteration.
            // After this call, `param_ptr` is invalidated — but we have already
            // copied the payload above.
            library.free_last_callback(self.pipe)?;

            out.push(callback_decode::decode(crate::raw_callback::RawCallback {
                id,
                payload,
            }));
        }

        Ok(out)
    }
}

/// Connects to Steam.
///
/// Pass `0` for no specific app context (suitable for picker / splash screens
/// that only need the user's SteamID; per-app stat reads will return
/// `CallFailed`). Pass a real Steam app ID (e.g. `105600` for Terraria) when
/// entering app-specific flows that need `GetStat` / `SetStat` /
/// `IndicateAchievementProgress` to operate against that app's stat counters.
///
/// When `app_id != 0` this function writes `SteamAppId` to the process
/// environment *before* loading `steamclient.so`. Steam reads this variable
/// exactly once during its first-touch library initialisation — the call that
/// triggers `loader::shared()` / `dlopen`. After the library is loaded the
/// variable has no further effect in the same process.
///
/// When `app_id == 0` the environment is left untouched (matching SAM's
/// `if (appId != 0) SetEnvironmentVariable(...)` guard).
///
/// # Concurrency
///
/// When `app_id != 0` this function mutates the process-wide environment via
/// `std::env::set_var`, which is `unsafe` in Rust 2024 because it races with
/// concurrent reads from other threads. Call `connect` before spawning any
/// thread that reads `std::env`. The library load is single-shot and
/// idempotent — the *first* call with a non-zero `app_id` sets the app
/// context for the entire process lifetime.
///
/// # Errors
///
/// - [`SteamError::SteamInstallNotFound`] — could not locate `steamclient.so`.
/// - [`SteamError::SteamNotRunning`] — `CreateSteamPipe` or `ConnectToGlobalUser`
///   returned 0, meaning no Steam process is listening on the IPC socket.
/// - [`SteamError::InterfaceUnavailable`] — Steam returned a null pointer for
///   the `SteamUser012` or `STEAMUSERSTATS_INTERFACE_VERSION013` interface.
pub fn connect(app_id: u32) -> Result<Client, SteamError> {
    if app_id != 0 {
        // SAFETY: `std::env::set_var` is `unsafe` in Rust 2024 because it
        // mutates process-wide state without synchronisation. The caller
        // contract (documented above) requires that this function is called
        // before any other thread reads `std::env`. The value written is a
        // pure ASCII decimal — no NUL bytes, no invalid UTF-8 — and
        // `SteamAppId` is a fixed ASCII key. Steam reads the env once during
        // the library's first-touch init triggered by `loader::shared()` below;
        // setting it before that call is the only correct ordering.
        unsafe {
            std::env::set_var("SteamAppId", app_id.to_string());
        }
    }

    let library = loader::shared()?;
    let steam_client = library.create_interface(STEAM_CLIENT_VERSION)?;

    // SAFETY: `steam_client` was returned by `CreateInterface("SteamClient018")`,
    // whose contract guarantees the returned object's first machine word
    // points to a vtable laid out as `ISteamClient018`. The vtable pointer
    // is read once and dereferenced for the immediate call only.
    let pipe = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).create_steam_pipe)(steam_client)
    };
    if pipe == 0 {
        return Err(SteamError::SteamNotRunning);
    }

    // SAFETY: `pipe` is the handle Steam returned in the call above and is
    // valid for use with this same `steam_client`.
    let user = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).connect_to_global_user)(steam_client, pipe)
    };
    if user == 0 {
        // SAFETY: must release the pipe Steam just gave us before bailing,
        // otherwise we leak IPC state in the steamclient process.
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

    // SAFETY: `user` and `pipe` are live handles from this same
    // `steam_client`. `version.as_ptr()` is a NUL-terminated C string that
    // outlives the call. The returned pointer is to a Steam-owned object
    // whose vtable matches `ISteamUser012` because we asked for that
    // version string.
    let steam_user = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_user)(steam_client, user, pipe, version.as_ptr())
    };
    if steam_user.is_null() {
        // SAFETY: same pipe/user handles, release in reverse-init order.
        unsafe {
            let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
            ((*vtbl).release_user)(steam_client, pipe, user);
            ((*vtbl).release_steam_pipe)(steam_client, pipe);
        }
        return Err(SteamError::InterfaceUnavailable {
            version: STEAM_USER_VERSION.to_owned(),
        });
    }

    // SAFETY: `steam_user` was vended as `SteamUser012`, so its vtable has
    // the `ISteamUser012` layout. On SysV-x64 the 8-byte CSteamID aggregate
    // is returned in RAX rather than via a hidden out-pointer, so the slot
    // is declared as `-> u64` for the Linux backend.
    let steam_id = unsafe {
        let vtbl = opaque::vtable::<ISteamUser012>(steam_user);
        ((*vtbl).get_steam_id)(steam_user)
    };

    let stats_version = CString::new(STEAM_USER_STATS_VERSION).map_err(|_| {
        SteamError::InvalidInterfaceVersion {
            version: STEAM_USER_STATS_VERSION.to_owned(),
        }
    })?;

    // SAFETY: `user` and `pipe` are live handles from this `steam_client`.
    // `stats_version.as_ptr()` is a NUL-terminated C string outliving the call.
    // The returned pointer is to a Steam-owned `ISteamUserStats013` object;
    // its vtable layout matches the struct we declared for that version string.
    // Sub-interface pointers are not released — only user and pipe handles are.
    let steam_user_stats = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_user_stats)(steam_client, user, pipe, stats_version.as_ptr())
    };
    if steam_user_stats.is_null() {
        // SAFETY: release in reverse-init order before returning the error.
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

    // SAFETY: `user` and `pipe` are live handles from this `steam_client`.
    // `apps_version.as_ptr()` is a NUL-terminated C string outliving the call.
    // The returned pointer is to a Steam-owned `ISteamApps001` object whose
    // vtable layout matches the struct declared for that version string.
    // Sub-interface pointers are not released — only user and pipe handles are.
    // A null return is non-fatal: `app_name()` guards on `steam_apps.is_null()`.
    let steam_apps = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_version.as_ptr())
    };

    let apps_008_version =
        CString::new(STEAM_APPS_008_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_APPS_008_VERSION.to_owned(),
        })?;

    // SAFETY: same handle/lifetime contract as the apps001 load above.
    // The returned pointer is to a Steam-owned `ISteamApps008` object whose
    // vtable layout matches the struct declared for that version string.
    // Null return is non-fatal: `is_subscribed_app()` guards on null.
    let steam_apps_008 = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_apps)(steam_client, user, pipe, apps_008_version.as_ptr())
    };

    let utils_version =
        CString::new(STEAM_UTILS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_UTILS_VERSION.to_owned(),
        })?;

    // SAFETY: `pipe` is the live HSteamPipe handle from this `steam_client`.
    // `utils_version.as_ptr()` is a NUL-terminated C string outliving the call.
    // `GetISteamUtils` takes only `(this, pipe, version)` — no user handle —
    // matching the SteamClient018 interface definition (slot 9). The returned
    // pointer is to a Steam-owned `ISteamUtils005` object whose vtable layout
    // matches the struct declared for that version string. Sub-interface
    // pointers are not released — only user and pipe handles are. A null return
    // is non-fatal: `get_image()` guards on `steam_utils.is_null()`.
    let steam_utils = unsafe {
        let vtbl = opaque::vtable::<ISteamClient018>(steam_client);
        ((*vtbl).get_isteam_utils)(steam_client, pipe, utils_version.as_ptr())
    };

    let friends_version =
        CString::new(STEAM_FRIENDS_VERSION).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: STEAM_FRIENDS_VERSION.to_owned(),
        })?;

    // SAFETY: `user` and `pipe` are live handles from this `steam_client`.
    // `friends_version.as_ptr()` is a NUL-terminated C string outliving the call.
    // `GetISteamFriends` (slot 8 of ISteamClient018) takes `(this, user, pipe, version)`.
    // The returned pointer is to a Steam-owned `ISteamFriends009` object whose
    // vtable layout matches the struct declared for that version string.
    // A null return is non-fatal: `persona_name()` and `user_avatar()` guard
    // on `steam_friends.is_null()`.
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
        // SAFETY: Drop releases the user handle, then the pipe handle.
        // Sub-interface object pointers (`steam_user`) are owned by
        // `steamclient.so` and are not separately released. The library
        // itself is owned by a process-global `OnceLock` and is never
        // unloaded — Steam's internal IPC/dispatch threads hold code
        // pointers into the library's text segment, so calling `dlclose`
        // would crash them on their next instruction. All raw pointers
        // and handles touched here were minted by the same `steam_client`
        // instance during `connect` and remain valid because nothing else
        // has run on this thread that could invalidate them.
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
    // Steam's GetUserDataFolder returns paths like:
    //   <steam_root>/userdata/<steamid3>/<app_id>/local
    // when the pipe was opened with a specific app_id (or 0 for probes).
    // The number of trailing components after `userdata` is therefore not
    // fixed. Find the `userdata` component and return its parent.
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
