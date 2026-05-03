use core::marker::PhantomData;
use core::ptr::addr_of;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::{
    CallbackMessage, HSteamPipe, HSteamUser, ISteamApps001, ISteamClient018, ISteamUser012,
    ISteamUtils005,
};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};
use crate::stat_schema::{StatDescriptor, load as load_stat_descriptors};
use crate::steam_callback::{SteamCallback, callback_decode};
use crate::user_stats::UserStats;

const STEAM_CLIENT_VERSION: &str = "SteamClient018";
const STEAM_USER_VERSION: &str = "SteamUser012";
const STEAM_USER_STATS_VERSION: &str = "STEAMUSERSTATS_INTERFACE_VERSION013";
const STEAM_APPS_VERSION: &str = "STEAMAPPS_INTERFACE_VERSION001";
const STEAM_UTILS_VERSION: &str = "SteamUtils005";

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
    _steam_user: RawInterface,
    steam_user_stats: RawInterface,
    steam_apps: RawInterface,
    steam_utils: RawInterface,
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

    /// Returns the human-readable display name for the connected app, or `None`
    /// when `app_id` is 0, Steam has no entry for the app, or the returned
    /// buffer is empty.
    ///
    /// Data is served from Steam's local appcache — no network request is made.
    /// The returned string is owned and heap-allocated; no pointer into Steam's
    /// memory is retained after this call returns.
    pub fn app_name(&self) -> Option<String> {
        if self.app_id == 0 || self.steam_apps.is_null() {
            return None;
        }

        let key = c"name";
        let mut buf = [0u8; 1024];

        // SAFETY: `self.steam_apps` was vended as "STEAMAPPS_INTERFACE_VERSION001"
        // so its vtable layout matches `ISteamApps001`. `self.app_id` is the
        // app-specific ID stored at connect time. `key.as_ptr()` is a static
        // NUL-terminated C string. `buf.as_mut_ptr()` is a valid, aligned,
        // uniquely-owned buffer of `buf.len()` bytes — Steam writes at most
        // `value_length` bytes into it. The written bytes are read immediately
        // after the call before any other Steam call can occur. No pointer into
        // Steam-owned memory is retained.
        let written = unsafe {
            let vtbl = opaque::vtable::<ISteamApps001>(self.steam_apps);
            ((*vtbl).get_app_data)(
                self.steam_apps,
                self.app_id,
                key.as_ptr(),
                buf.as_mut_ptr().cast::<core::ffi::c_char>(),
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

    Ok(Client {
        steam_client,
        _steam_user: steam_user,
        steam_user_stats,
        steam_apps,
        steam_utils,
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
        // Sub-interface object pointers (`_steam_user`) are owned by
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
