use core::ffi::c_void;
use core::marker::PhantomData;
use std::ffi::CString;

use crate::error::SteamError;
use crate::ffi::interfaces::ISteamUserStats013;
use crate::ffi::opaque::{self, RawInterface};

/// Safe wrapper around the `ISteamUserStats013` sub-interface.
///
/// Obtain an instance via [`crate::Client::user_stats`].
///
/// # Lifetime
///
/// `UserStats<'a>` borrows the `Client` that produced it. All raw pointers
/// returned by Steam during calls on this type are valid only until the next
/// Steam call on the same pipe — they are copied immediately and never stored.
///
/// # Threading
///
/// `UserStats` is `!Send` and `!Sync` because the underlying pipe is
/// thread-local to the thread that called `connect()`.
///
/// # Stats availability
///
/// Until `RequestUserStats` completes (Round 2), `get_stat_*` methods will
/// return Steam's defaults (typically 0 / 0.0 / false). `set_*` calls
/// stage the values locally; they are not persisted until `store_stats`
/// succeeds and the resulting `UserStatsStored` callback is received.
pub struct UserStats<'a> {
    raw: RawInterface,
    _client: PhantomData<&'a c_void>,
    _not_send: PhantomData<*const ()>,
}

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

    /// Returns whether the local user has unlocked the named achievement.
    ///
    /// Returns `SteamError::CallFailed` if Steam does not recognise the
    /// achievement name (e.g. schema not loaded yet — call
    /// `RequestUserStats` first in Round 2).
    pub fn get_achievement(&self, name: &str) -> Result<bool, SteamError> {
        let cname = Self::cname(name)?;
        let mut achieved = false;
        // SAFETY: `self.raw` was vended as `STEAMUSERSTATS_INTERFACE_VERSION013`,
        // so its vtable matches `ISteamUserStats013` (slot 5 = GetAchievement).
        // `cname` is a valid NUL-terminated UTF-8 string that outlives this call.
        // `achieved` is a stack bool; Steam writes through the pointer before returning.
        // SysV-x64: `this` passes in RDI, `name` in RSI, `achieved` (out-ptr) in RDX.
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

    /// Marks the named achievement as unlocked in the local staging area.
    ///
    /// Call `store_stats` afterwards to persist the change.
    pub fn set_achievement(&self, name: &str) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: slot 6 = SetAchievement. `this` + NUL-terminated name.
        // Returns bool; false means Steam rejected the name (e.g. schema not loaded).
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

    /// Clears (locks) the named achievement in the local staging area.
    ///
    /// Call `store_stats` afterwards to persist the change.
    pub fn clear_achievement(&self, name: &str) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: slot 7 = ClearAchievement. Same invariants as set_achievement.
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

    /// Returns `(achieved, unix_timestamp)` for the named achievement.
    ///
    /// `unix_timestamp` is `0` when the achievement has not been unlocked.
    pub fn achievement_and_unlock_time(&self, name: &str) -> Result<(bool, u32), SteamError> {
        let cname = Self::cname(name)?;
        let mut achieved = false;
        let mut unlock_time: u32 = 0;
        // SAFETY: slot 8 = GetAchievementAndUnlockTime. Both out-pointers are
        // stack-allocated and outlive the call. Steam writes through them only
        // when it returns true.
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

    /// Returns the value of the named integer stat from the local cache.
    pub fn get_stat_int(&self, name: &str) -> Result<i32, SteamError> {
        let cname = Self::cname(name)?;
        let mut value: i32 = 0;
        // SAFETY: slot 1 = GetStatInteger. `value` is a stack i32; Steam writes
        // through the pointer. Returns false if the name is unknown or stats
        // are not loaded yet.
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

    /// Returns the value of the named float stat from the local cache.
    pub fn get_stat_float(&self, name: &str) -> Result<f32, SteamError> {
        let cname = Self::cname(name)?;
        let mut value: f32 = 0.0;
        // SAFETY: slot 0 = GetStatFloat. `value` is a stack f32; Steam writes
        // through the pointer.
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

    /// Stages a new value for the named integer stat.
    ///
    /// Call `store_stats` to persist.
    pub fn set_stat_int(&self, name: &str, value: i32) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: slot 3 = SetStatInteger. Value passed by register (i32).
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

    /// Stages a new value for the named float stat.
    ///
    /// Call `store_stats` to persist.
    pub fn set_stat_float(&self, name: &str, value: f32) -> Result<(), SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: slot 2 = SetStatFloat. Value passed by XMM0 register (f32 by value).
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

    /// Commits all staged stat and achievement changes to the Steam backend.
    ///
    /// Steam will emit a `UserStatsStored` callback after persistence succeeds.
    /// Handling that callback is deferred to Round 2.
    ///
    /// Returns `CallFailed` only when Steam's internal write fails (e.g. offline,
    /// or no stats staged). A `false` return does not mean data was lost —
    /// retrying after a successful `RequestUserStats` round will re-stage the data.
    pub fn store_stats(&self) -> Result<(), SteamError> {
        // SAFETY: slot 9 = StoreStats. No parameters beyond `this`.
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

    /// Returns the number of achievements defined in the schema for this app.
    pub fn num_achievements(&self) -> Result<u32, SteamError> {
        // SAFETY: slot 13 = GetNumAchievements. Returns u32 in RAX.
        // Cannot fail (returns 0 when schema is not loaded, which is not an error state).
        let count = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_num_achievements)(self.raw)
        };
        Ok(count)
    }

    /// Returns the API name of the achievement at the given index.
    ///
    /// Returns `AchievementNotFound` when `index` is out of range or the schema
    /// is not loaded (Steam returns a null pointer in both cases).
    pub fn achievement_name(&self, index: u32) -> Result<String, SteamError> {
        // SAFETY: slot 14 = GetAchievementName. Returns a pointer to a static
        // string buffer owned by `steamclient.so`. The pointer is valid until
        // the next Steam call on this pipe, so we copy into an owned String
        // before returning. Null indicates index-out-of-range or schema not loaded.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_name)(self.raw, index)
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: format!("<index {index}>"),
            });
        }
        // SAFETY: Steam guarantees the returned pointer is a NUL-terminated
        // UTF-8 string when non-null. We copy before any further Steam call.
        let name = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(name)
    }

    /// Returns Steam's opaque image handle for the achievement icon.
    ///
    /// Pass the returned handle to `ISteamUtils::GetImageRGBA` (not yet
    /// implemented) to retrieve the raw pixel data.
    ///
    /// Returns `0` when the icon has not been loaded yet (Steam delivers it
    /// asynchronously the first time it is requested). In that case subscribe
    /// to the `UserAchievementIconFetched` callback (Round 2+).
    pub fn achievement_icon(&self, name: &str) -> Result<i32, SteamError> {
        let cname = Self::cname(name)?;
        // SAFETY: slot 10 = GetAchievementIcon. Returns i32 icon handle in RAX.
        // 0 means "not loaded yet" — not an error; caller must wait for callback.
        let handle = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_icon)(self.raw, cname.as_ptr())
        };
        Ok(handle)
    }

    /// Returns a display attribute for the named achievement.
    ///
    /// `attribute` must be one of `"name"`, `"desc"`, or `"hidden"` (as a
    /// stringified `"0"` / `"1"`). Steam serves these values from the locally
    /// cached schema; they are available without a network round-trip once the
    /// schema has been downloaded.
    ///
    /// Returns `AchievementNotFound` when Steam returns a null pointer, which
    /// happens when the achievement name is unknown or the schema is not yet
    /// loaded.
    pub fn achievement_display_attribute(
        &self,
        name: &str,
        attribute: &str,
    ) -> Result<String, SteamError> {
        let cname = Self::cname(name)?;
        let cattr = Self::cname(attribute)?;
        // SAFETY: slot 11 = GetAchievementDisplayAttribute. Returns a pointer
        // to a static string buffer owned by `steamclient.so`; valid until the
        // next Steam call on this pipe. We copy into an owned String immediately.
        // Null indicates unknown achievement or schema not loaded.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_display_attribute)(self.raw, cname.as_ptr(), cattr.as_ptr())
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: name.to_owned(),
            });
        }
        // SAFETY: Steam guarantees the returned pointer is a NUL-terminated
        // UTF-8 string when non-null. We copy before any further Steam call.
        let value = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(value)
    }
}
