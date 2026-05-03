use core::marker::PhantomData;
use core::ptr::addr_of;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::{
    CallbackMessage, HSteamPipe, HSteamUser, ISteamClient018, ISteamUser012,
};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};
use crate::steam_callback::{SteamCallback, callback_decode};
use crate::user_stats::UserStats;

const STEAM_CLIENT_VERSION: &str = "SteamClient018";
const STEAM_USER_VERSION: &str = "SteamUser012";
const STEAM_USER_STATS_VERSION: &str = "STEAMUSERSTATS_INTERFACE_VERSION013";

pub struct Client {
    steam_client: RawInterface,
    _steam_user: RawInterface,
    steam_user_stats: RawInterface,
    pipe: HSteamPipe,
    user: HSteamUser,
    steam_id: u64,
    _not_send: PhantomData<*const ()>,
}

impl Client {
    pub fn steam_id(&self) -> u64 {
        self.steam_id
    }

    /// Returns the user-stats sub-interface for achievement and stat operations.
    ///
    /// Until `RequestUserStats` completes (Round 2), `get_*` methods return
    /// Steam's defaults (typically 0 / `false`). `set_*` calls stage changes
    /// locally; `store_stats` must be called to persist them.
    pub fn user_stats(&self) -> UserStats<'_> {
        UserStats::from_raw(self.steam_user_stats)
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

pub fn connect() -> Result<Client, SteamError> {
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

    Ok(Client {
        steam_client,
        _steam_user: steam_user,
        steam_user_stats,
        pipe,
        user,
        steam_id,
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
