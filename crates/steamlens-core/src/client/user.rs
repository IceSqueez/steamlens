use core::ffi::c_char;
use std::path::PathBuf;

use crate::client::internal::nul_terminated_str;
use crate::error::SteamError;
use crate::ffi::interfaces::ISteamUser023;
use crate::ffi::opaque::{self, RawInterface};

pub(super) struct User {
    pub(super) steam_user: RawInterface,
    pub(super) steam_id: u64,
    pub(super) app_id: u32,
}

impl User {
    pub(super) fn steam_id(&self) -> u64 {
        self.steam_id
    }

    pub(super) fn app_id(&self) -> u32 {
        self.app_id
    }

    pub(super) fn get_player_steam_level(&self) -> Option<u32> {
        // SAFETY: `steam_user` is the live `SteamUser023` interface acquired
        // in `establish`; vtable slot 24 = `GetPlayerSteamLevel`; called on
        // the `!Send` owner thread; primitive `i32` return is ABI-safe.
        tracing::trace!("user: get_player_steam_level pre");
        let level = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(self.steam_user);
            ((*vtbl).get_player_steam_level)(self.steam_user)
        };
        tracing::trace!(level, "user: get_player_steam_level post");
        if level < 0 { None } else { Some(level as u32) }
    }

    pub(super) fn user_data_folder(&self) -> Result<PathBuf, SteamError> {
        const USER_DATA_FOLDER_BUFFER_LEN: usize = 1024;
        let mut buf = [0u8; USER_DATA_FOLDER_BUFFER_LEN];

        // SAFETY: live `SteamUser023`; slot 6 = `GetUserDataFolder`; Steam
        // writes at most `buf.len()` bytes into the stack buffer.
        tracing::trace!("user: get_user_data_folder pre");
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(self.steam_user);
            ((*vtbl).get_user_data_folder)(
                self.steam_user,
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len() as i32,
            )
        };
        tracing::trace!(ok, "user: get_user_data_folder post");

        if !ok {
            return Err(SteamError::UserDataFolderUnavailable);
        }

        let path_str = nul_terminated_str(&buf).ok_or(SteamError::UserDataFolderUnavailable)?;

        Ok(PathBuf::from(path_str))
    }

    pub(super) fn steam_root(&self) -> Result<PathBuf, SteamError> {
        let path = self.user_data_folder()?;
        strip_userdata_suffix(path)
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
        let path = PathBuf::from("/home/x/.local/share/Steam/userdata/12345");
        let root = strip_userdata_suffix(path).unwrap();
        assert_eq!(root, PathBuf::from("/home/x/.local/share/Steam"));
    }

    #[test]
    fn steam_root_from_real_steam_format_with_app_id_and_local() {
        let path = PathBuf::from("/home/x/.local/share/Steam/userdata/12345/0/local");
        let root = strip_userdata_suffix(path).unwrap();
        assert_eq!(root, PathBuf::from("/home/x/.local/share/Steam"));
    }

    #[test]
    fn steam_root_from_real_format_with_specific_app_id() {
        let path = PathBuf::from("/opt/steam/userdata/9876/480/remote/cloud");
        let root = strip_userdata_suffix(path).unwrap();
        assert_eq!(root, PathBuf::from("/opt/steam"));
    }

    #[test]
    fn steam_root_from_short_path() {
        let path = PathBuf::from("/opt/steam/userdata/9876");
        let root = strip_userdata_suffix(path).unwrap();
        assert_eq!(root, PathBuf::from("/opt/steam"));
    }

    #[test]
    fn steam_root_missing_userdata_component_returns_error() {
        let path = PathBuf::from("/home/x/.local/share/Steam/12345");
        let err = strip_userdata_suffix(path).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }

    #[test]
    fn steam_root_wrong_parent_name_returns_error() {
        let path = PathBuf::from("/home/x/notsteam/notuserdata/12345");
        let err = strip_userdata_suffix(path).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }

    #[test]
    fn steam_root_only_two_components_returns_error() {
        let path = PathBuf::from("userdata/12345");
        let err = strip_userdata_suffix(path).unwrap_err();
        assert!(matches!(err, SteamError::MalformedUserDataPath { .. }));
    }
}
