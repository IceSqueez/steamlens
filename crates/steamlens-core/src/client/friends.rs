use crate::ffi::interfaces::ISteamFriends009;
use crate::ffi::opaque::{self, RawInterface};

pub(super) struct Friends {
    pub(super) steam_friends: RawInterface,
}

impl Friends {
    pub(super) fn persona_name(&self) -> Option<String> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: `steam_friends` was obtained from `GetISteamFriends("SteamFriends009")`
        // in `SteamConnection::establish`; the pipe is alive for the lifetime of the owning
        // `Client`; ISteamFriends009 slot 0 returns a NUL-terminated UTF-8 string valid
        // until the next Steam call on this pipe — we copy it immediately; SysV-x64 ABI.
        let raw_ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_persona_name)(self.steam_friends)
        };
        if raw_ptr.is_null() {
            return None;
        }
        // SAFETY: Steam returned a non-null NUL-terminated UTF-8 pointer valid until
        // the next Steam call; we copy it to an owned String before any further call.
        let name = unsafe { std::ffi::CStr::from_ptr(raw_ptr) }
            .to_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)?;
        Some(name)
    }
}
