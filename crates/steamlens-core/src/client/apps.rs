use core::ffi::c_char;

use crate::ffi::interfaces::{ISteamApps001, ISteamApps008};
use crate::ffi::opaque::{self, RawInterface};

pub(super) struct Apps {
    pub(super) steam_apps: RawInterface,
    pub(super) steam_apps_008: RawInterface,
    pub(super) app_id: u32,
}

impl Apps {
    pub(super) fn app_name_for(&self, app_id: u32) -> Option<String> {
        self.get_app_data_raw(app_id, c"name")
    }

    pub(super) fn app_name(&self) -> Option<String> {
        self.app_name_for(self.app_id)
    }

    pub(super) fn app_type(&self, app_id: u32) -> Option<String> {
        self.get_app_data_raw(app_id, c"type")
    }

    pub(super) fn get_app_data(&self, app_id: u32, key: &core::ffi::CStr) -> Option<String> {
        self.get_app_data_raw(app_id, key)
    }

    pub(super) fn is_subscribed_app(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: `steam_apps_008` was obtained from `GetISteamApps("STEAMAPPS_INTERFACE_VERSION008")`
        // in `SteamConnection::establish`; the pipe is alive for the lifetime of the owning
        // `Client`; ISteamApps008 vtable layout is stable; SysV-x64 ABI.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_subscribed_app)(self.steam_apps_008, app_id)
        }
    }

    pub(super) fn is_app_installed(&self, app_id: u32) -> bool {
        if self.steam_apps_008.is_null() {
            return false;
        }
        // SAFETY: same as `is_subscribed_app`.
        unsafe {
            let vtbl = opaque::vtable::<ISteamApps008>(self.steam_apps_008);
            ((*vtbl).is_app_installed)(self.steam_apps_008, app_id)
        }
    }

    fn get_app_data_raw(&self, app_id: u32, key: &core::ffi::CStr) -> Option<String> {
        if app_id == 0 || self.steam_apps.is_null() {
            return None;
        }
        let mut buf = [0u8; 1024];
        // SAFETY: `steam_apps` was obtained from `GetISteamApps("STEAMAPPS_INTERFACE_VERSION001")`
        // in `SteamConnection::establish`; the pipe is alive for the lifetime of the owning
        // `Client`; `key` is a static NUL-terminated C string; Steam writes into `buf` (stack)
        // and we copy out before any further Steam call; SysV-x64 ABI.
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
}
