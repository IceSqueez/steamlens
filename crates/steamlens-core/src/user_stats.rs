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

// Impl-level safety invariant for every `unsafe` block in this `impl`:
//
// `self.raw` is a pointer vended by Steam via `GetISteamUserStats` against
// the version string `STEAMUSERSTATS_INTERFACE_VERSION013`.  Steam guarantees
// its vtable layout matches `ISteamUserStats013` — dispatch is positional
// (slot N = method M as declared in that interface).  The pointer remains
// valid for the lifetime `'a` (tied to the `&'a Client` that owns the pipe).
//
// Calling convention on the current target (Linux x86_64 SysV-x64): `this`
// is the first integer argument (RDI); remaining arguments follow in RSI,
// RDX, RCX, R8, R9.  No `thiscall` is required (that applies only to the
// Windows x86 backend, not yet implemented).
//
// Per-block comments below state the specific vtable slot and any additional
// invariants (lifetime of string arguments, out-pointer validity, etc.) on
// top of this base.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 5 = GetAchievement.  `cname` is NUL-terminated and held alive by
        // the binding for the duration of the call.  `achieved` is a stack-allocated
        // bool; Steam writes through the pointer before returning.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 6 = SetAchievement.  `cname` is NUL-terminated and held alive by
        // the binding for the duration of the call.  Returns bool; false means
        // Steam rejected the name (schema not loaded or name unknown).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 7 = ClearAchievement.  `cname` is NUL-terminated and held alive by
        // the binding for the duration of the call.  Returns bool; false means
        // Steam rejected the name (schema not loaded or name unknown).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 8 = GetAchievementAndUnlockTime.  `cname` is NUL-terminated and
        // held alive by the binding.  `achieved` and `unlock_time` are
        // stack-allocated out-params; Steam writes through them only when it
        // returns true.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 1 = GetStatInteger.  `cname` is NUL-terminated and held alive by
        // the binding.  `value` is a stack i32; Steam writes through the pointer
        // before returning.  Returns false when the name is unknown or stats are
        // not loaded yet.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 0 = GetStatFloat.  `cname` is NUL-terminated and held alive by
        // the binding.  `value` is a stack f32; Steam writes through the pointer
        // before returning.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 3 = SetStatInteger.  `cname` is NUL-terminated and held alive by
        // the binding.  `value` is passed by register (i32 in RDX on SysV-x64).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 2 = SetStatFloat.  `cname` is NUL-terminated and held alive by
        // the binding.  `value` is passed by XMM register (f32 on SysV-x64).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 9 = StoreStats.  No additional parameters beyond `this`.
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 13 = GetNumAchievements.  No additional parameters.  Returns u32
        // in RAX; 0 when the schema is not yet loaded (not an error — caller
        // should call RequestUserStats first).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 14 = GetAchievementName.  Returns a pointer to a static string
        // buffer owned by `steamclient.so`; valid until the next Steam call on
        // this pipe.  Null indicates index-out-of-range or schema not loaded.
        // The subsequent `CStr::from_ptr` block copies the bytes before any
        // further Steam call can invalidate the buffer.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_name)(self.raw, index)
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: format!("<index {index}>"),
            });
        }
        // SAFETY: Non-null was verified above.  Steam guarantees a valid
        // NUL-terminated UTF-8 string in this buffer for the lifetime noted in
        // the vtable-call block.  We copy immediately; no further Steam call
        // intervenes.
        let name = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(name)
    }

    /// Queues an async request to load stats and achievements for the given Steam user.
    ///
    /// Returns immediately. The result arrives as a [`crate::SteamCallback::UserStatsReceived`]
    /// in a subsequent [`crate::Client::poll_callbacks`] call. Until that callback arrives with
    /// `result.is_ok()`, all `get_stat_*` and `get_achievement` methods return Steam's defaults
    /// (0 / `false`).
    ///
    /// Pass the Steam ID of the local user (available via [`crate::Client::steam_id`]).
    ///
    /// Returns `CallFailed` when Steam returns an invalid call handle (0), which happens when
    /// Steam is offline or the user is not signed in.
    pub fn request_user_stats(&self, steam_id: u64) -> Result<(), SteamError> {
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 15 = RequestUserStats. `steam_id` is a plain u64 register argument (RSI on
        // SysV-x64). Returns a SteamAPICall_t (u64) call handle; 0 means Steam rejected the
        // request. We do not track the call handle — typed callbacks arrive via poll_callbacks.
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

    /// Resets all stats (and optionally all achievements) for the current app
    /// to their default values in the local staging area.
    ///
    /// If `achievements_too` is `true`, all achievements are also cleared as a
    /// side effect — achievements that depend on a stat counter will remain
    /// locked even after the game re-reads those counters.
    ///
    /// This is the canonical "wipe everything" call. Per-achievement
    /// [`Self::clear_achievement`] only clears the achievement flag without
    /// touching the underlying stat — so a stat-driven achievement (e.g.
    /// "complete 25 quests") immediately re-unlocks the next time the game
    /// runs and observes the unchanged stat counter. Use this method for a
    /// true reset that survives subsequent game launches.
    ///
    /// Like all writes, the change is staged locally until [`Self::store_stats`]
    /// is called.
    pub fn reset_all_stats(&self, achievements_too: bool) -> Result<(), SteamError> {
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 20 = ResetAllStats.  `achievements_too` is a bool passed in RSI
        // (SysV-x64); Steam marshals it as a 1-byte value internally.  Returns
        // bool in RAX; false means Steam rejected the call (e.g. stats not yet
        // loaded — call RequestUserStats first).
        let ok = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).reset_all_stats)(self.raw, achievements_too)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::CallFailed {
                method: "ResetAllStats",
            })
        }
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 10 = GetAchievementIcon.  `cname` is NUL-terminated and held alive
        // by the binding.  Returns i32 icon handle in RAX; 0 means the icon has
        // not been fetched yet (not an error — caller subscribes to the
        // UserAchievementIconFetched callback, Round 2+).
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
        // SAFETY: Impl-level invariant holds (see above).
        // Slot 11 = GetAchievementDisplayAttribute.  Both `cname` and `cattr`
        // are NUL-terminated and held alive by their bindings.  Returns a pointer
        // to a static string buffer owned by `steamclient.so`; valid until the
        // next Steam call on this pipe.  The subsequent `CStr::from_ptr` block
        // copies the bytes before any further Steam call can invalidate the buffer.
        // Null indicates unknown achievement name or schema not loaded.
        let ptr = unsafe {
            let vtbl = opaque::vtable::<ISteamUserStats013>(self.raw);
            ((*vtbl).get_achievement_display_attribute)(self.raw, cname.as_ptr(), cattr.as_ptr())
        };
        if ptr.is_null() {
            return Err(SteamError::AchievementNotFound {
                name: name.to_owned(),
            });
        }
        // SAFETY: Non-null was verified above.  Steam guarantees a valid
        // NUL-terminated UTF-8 string in this buffer for the lifetime noted in
        // the vtable-call block.  We copy immediately; no further Steam call
        // intervenes.
        let value = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        Ok(value)
    }
}
