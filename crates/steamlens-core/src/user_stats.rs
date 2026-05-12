use core::ffi::c_void;
use core::marker::PhantomData;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::ISteamUserStats013;
use crate::ffi::opaque::{self, RawInterface};

/// `!Send` / `!Sync`. Steam-owned pointers returned by methods are valid
/// only until the next Steam call on this pipe; this wrapper copies them
/// out before returning.
pub struct UserStats<'a> {
    raw: RawInterface,
    _client: PhantomData<&'a c_void>,
    _not_send: PhantomData<*const ()>,
}

// Every `unsafe` block in this impl shares an invariant: `self.raw` is
// the live `ISteamUserStats013` vtable pointer vended against
// `STEAMUSERSTATS_INTERFACE_VERSION013` with SysV-x64 dispatch (`this` in
// RDI). Per-block SAFETY notes only call out the extra invariants
// introduced by that block (argument lifetime, out-pointer validity).
impl<'a> UserStats<'a> {
    pub(crate) fn from_raw(raw: RawInterface) -> Self {
        Self {
            raw,
            _client: PhantomData,
            _not_send: PhantomData,
        }
    }

    fn cname(s: &str) -> Result<CString, SteamError> {
        CString::new(s).map_err(|source| SteamError::InvalidString { source })
    }

    pub fn get_achievement(&self, name: &str) -> Result<bool, SteamError> {
        let cname = Self::cname(name)?;
        let mut achieved = false;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement)(self.raw, cname.as_ptr(), &mut achieved)
        };
        if ok {
            Ok(achieved)
        } else {
            Err(SteamError::CallFailed {
                method: "GetAchievement",
            })
        }
    }

    pub fn set_achievement(&self, name: &str) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).set_achievement)(self.raw, cname.as_ptr())
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "SetAchievement",
            })
        }
    }

    pub fn clear_achievement(&self, name: &str) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).clear_achievement)(self.raw, cname.as_ptr())
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "ClearAchievement",
            })
        }
    }

    /// Returns `(achieved, unix_timestamp)` — `unlock_time` is `0` when
    /// the achievement is locked.
    pub fn achievement_and_unlock_time(&self, name: &str) -> Result<(bool, u32), SteamError> {
        let cname = Self::cname(name)?;
        let mut achieved = false;
        let mut unlock_time: u32 = 0;
        // SAFETY: `cname` outlives the call; out-params are written only
        // when the call returns `true`.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_and_unlock_time)(
                self.raw,
                cname.as_ptr(),
                &mut achieved,
                &mut unlock_time,
            )
        };
        if ok {
            Ok((achieved, unlock_time))
        } else {
            Err(SteamError::CallFailed {
                method: "GetAchievementAndUnlockTime",
            })
        }
    }

    pub fn get_stat_int(&self, name: &str) -> Result<i32, SteamError> {
        let cname = Self::cname(name)?;
        let mut value: i32 = 0;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_stat_int)(self.raw, cname.as_ptr(), &mut value)
        };
        if ok {
            Ok(value)
        } else {
            Err(SteamError::CallFailed {
                method: "GetStatInt",
            })
        }
    }

    pub fn get_stat_float(&self, name: &str) -> Result<f32, SteamError> {
        let cname = Self::cname(name)?;
        let mut value: f32 = 0.0;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_stat_float)(self.raw, cname.as_ptr(), &mut value)
        };
        if ok {
            Ok(value)
        } else {
            Err(SteamError::CallFailed {
                method: "GetStatFloat",
            })
        }
    }

    pub fn set_stat_int(&self, name: &str, value: i32) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).set_stat_int)(self.raw, cname.as_ptr(), value)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "SetStatInt",
            })
        }
    }

    pub fn set_stat_float(&self, name: &str, value: f32) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).set_stat_float)(self.raw, cname.as_ptr(), value)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "SetStatFloat",
            })
        }
    }

    pub fn store_stats(&self) -> Result<(), SteamError> {
        // SAFETY: see impl-level note.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).store_stats)(self.raw)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "StoreStats",
            })
        }
    }

    pub fn num_achievements(&self) -> u32 {
        // SAFETY: see impl-level note.
        unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_num_achievements)(self.raw)
        }
    }

    pub fn achievement_name(&self, index: u32) -> Result<String, SteamError> {
        // SAFETY: see impl-level note.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_name)(self.raw, index)
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: format!("<index {index}>"),
            });
        }
        // SAFETY: Steam owns a NUL-terminated UTF-8 buffer at `ptr` valid
        // until the next call on this pipe; we copy out immediately.
        let name = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(name)
    }

    /// Async — the result arrives as
    /// [`crate::SteamCallback::UserStatsReceived`]. Pass the local user's
    /// [`crate::Client::steam_id`].
    pub fn request_user_stats(&self, steam_id: u64) -> Result<(), SteamError> {
        // SAFETY: see impl-level note.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).request_user_stats)(self.raw, steam_id)
        };
        if handle == 0 {
            Err(SteamError::CallFailed {
                method: "RequestUserStats",
            })
        } else {
            Ok(())
        }
    }

    /// Drives the overlay's "X/Y" progress popup. Steam keeps this counter
    /// separately from regular stats — call with `current = 0` to clear the
    /// popup counter without a game-side write.
    pub fn indicate_achievement_progress(
        &self,
        name: &str,
        current: u32,
        max: u32,
    ) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).indicate_achievement_progress)(self.raw, cname.as_ptr(), current, max)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "IndicateAchievementProgress",
            })
        }
    }

    /// Returns a `SteamAPICall_t` — poll via
    /// [`crate::Client::poll_call_result`] with callback id `1110`.
    pub fn request_global_achievement_percentages(&self) -> Result<u64, SteamError> {
        // SAFETY: see impl-level note.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).request_global_achievement_percentages)(self.raw)
        };
        if handle == 0 {
            Err(SteamError::CallFailed {
                method: "RequestGlobalAchievementPercentages",
            })
        } else {
            Ok(handle)
        }
    }

    /// 0.0–100.0 — fraction of users who have unlocked the achievement.
    /// Wait for `GlobalAchievementPercentagesReady` after
    /// [`Self::request_global_achievement_percentages`] before reading,
    /// otherwise Steam returns `false` for every name.
    pub fn achievement_achieved_percent(&self, name: &str) -> Result<f32, SteamError> {
        let cname = Self::cname(name)?;
        let mut percent: f32 = 0.0;
        // SAFETY: `cname` outlives the call.
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_achieved_percent)(self.raw, cname.as_ptr(), &mut percent)
        };
        if ok {
            Ok(percent)
        } else {
            Err(SteamError::CallFailed {
                method: "GetAchievementAchievedPercent",
            })
        }
    }

    /// `0` while Steam is still fetching — subscribe to
    /// `UserAchievementIconFetched` to retry.
    pub fn achievement_icon(&self, name: &str) -> Result<i32, SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: `cname` outlives the call.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_icon)(self.raw, cname.as_ptr())
        };
        Ok(handle)
    }

    /// `attribute` is `"name"`, `"desc"`, or `"hidden"` (stringified
    /// `"0"`/`"1"`). Served from the local schema cache — no network.
    pub fn achievement_display_attribute(
        &self,
        name: &str,
        attribute: &str,
    ) -> Result<String, SteamError> {
        let cname = Self::cname(name)?;
        let cattr = Self::cname(attribute)?;
        // SAFETY: `cname` and `cattr` outlive the call.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_display_attribute)(self.raw, cname.as_ptr(), cattr.as_ptr())
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: name.to_owned(),
            });
        }
        // SAFETY: Steam owns a NUL-terminated UTF-8 buffer at `ptr` valid
        // until the next call on this pipe; we copy out immediately.
        let value = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(value)
    }
}
