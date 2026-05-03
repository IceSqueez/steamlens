use core::marker::PhantomData;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::{HSteamPipe, HSteamUser, ISteamClient018, ISteamUser012};
use crate::ffi::loader;
use crate::ffi::opaque::{self, RawInterface};

const STEAM_CLIENT_VERSION: &str = "SteamClient018";
const STEAM_USER_VERSION: &str = "SteamUser012";

pub struct Client {
    steam_client: RawInterface,
    _steam_user: RawInterface,
    pipe: HSteamPipe,
    user: HSteamUser,
    steam_id: u64,
    _not_send: PhantomData<*const ()>,
}

impl Client {
    pub fn steam_id(&self) -> u64 {
        self.steam_id
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

    Ok(Client {
        steam_client,
        _steam_user: steam_user,
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
