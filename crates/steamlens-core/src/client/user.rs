use core::ffi::c_char;
use std::path::PathBuf;

use crate::error::SteamError;
use crate::ffi::interfaces::{ISteamUser012, ISteamUser023};
use crate::ffi::opaque::{self, RawInterface};

pub(super) struct User {
    pub(super) steam_user: RawInterface,
    pub(super) steam_user_023: Option<RawInterface>,
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

    /// Returns `None` when `SteamUser023` is unavailable (very old Steam
    /// client) or when Steam returns a negative value (not a valid level).
    pub(super) fn get_player_steam_level(&self) -> Option<u32> {
        let iface = self.steam_user_023?;
        // SAFETY: `iface` was returned by `GetISteamUser("SteamUser023")` in
        // `SteamConnection::establish`; vtable layout matches the public Steamworks.NET
        // header (isteamuser.h slot 24); the pipe is alive (Client is `!Send` and must be
        // dropped before the pipe closes); SysV-x64 ABI — `this` in RDI, return in RAX as i32.
        let level = unsafe {
            let vtbl = opaque::vtable::<ISteamUser023>(iface);
            ((*vtbl).get_player_steam_level)(iface)
        };
        if level < 0 { None } else { Some(level as u32) }
    }

    pub(super) fn user_data_folder(&self) -> Result<PathBuf, SteamError> {
        let mut buf = [0u8; 1024];

        // SAFETY: `steam_user` was obtained from `GetISteamUser("SteamUser012")`;
        // the pipe is alive for the lifetime of the owning `Client`; Steam writes
        // at most `buf.len()` bytes into the stack buffer; SysV-x64 ABI.
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

    pub(super) fn steam_root(&self) -> Result<PathBuf, SteamError> {
        let udf = self.user_data_folder()?;
        strip_userdata_suffix(udf)
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
