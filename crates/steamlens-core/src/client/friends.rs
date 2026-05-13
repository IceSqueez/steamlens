use crate::client::Image;
use crate::ffi::interfaces::ISteamFriends009;
use crate::ffi::opaque::{self, RawInterface};

pub(super) struct Friends {
    pub(super) steam_friends: RawInterface,
    pub(super) steam_id: u64,
}

impl Friends {
    pub(super) fn nickname(&self) -> Option<String> {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: live `ISteamFriends009`; slot 0 returns a NUL-terminated
        // UTF-8 pointer valid until the next Steam call — we copy it
        // immediately on return.
        tracing::trace!("friends: get_persona_name pre");
        let raw_ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_persona_name)(self.steam_friends)
        };
        tracing::trace!(null = raw_ptr.is_null(), "friends: get_persona_name post");
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

    pub(super) fn user_avatar<F>(&self, get_image: F) -> Option<Image>
    where
        F: FnOnce(i32) -> Option<Image>,
    {
        if self.steam_friends.is_null() {
            return None;
        }
        // SAFETY: live `ISteamFriends009`, slot 26 = `GetMediumFriendAvatar`;
        // CSteamID is passed inline as a `u64` argument — input parameters are
        // ABI-safe on both MSVC and SysV.
        tracing::trace!(
            steam_id = self.steam_id,
            "friends: get_medium_friend_avatar pre"
        );
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamFriends009>(self.steam_friends);
            ((*vtbl).get_medium_friend_avatar)(self.steam_friends, self.steam_id)
        };
        tracing::trace!(handle, "friends: get_medium_friend_avatar post");
        if handle == 0 {
            return None;
        }
        get_image(handle)
    }
}
