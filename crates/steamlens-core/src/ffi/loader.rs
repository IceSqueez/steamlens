use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libloading::{Library, Symbol};

use crate::error::SteamError;
use crate::ffi::interfaces::{
    BGetCallbackFn, CallbackMessage, CreateInterfaceFn, FreeLastCallbackFn, HSteamPipe,
};
use crate::ffi::opaque::RawInterface;
use crate::paths;

pub struct SteamLibrary {
    handle: Library,
    b_get_callback_fn: BGetCallbackFn,
    free_last_callback_fn: FreeLastCallbackFn,
}

static STEAM_LIBRARY: OnceLock<SteamLibrary> = OnceLock::new();

pub fn shared() -> Result<&'static SteamLibrary, SteamError> {
    if let Some(lib) = STEAM_LIBRARY.get() {
        return Ok(lib);
    }
    let loaded = SteamLibrary::load()?;
    Ok(STEAM_LIBRARY.get_or_init(|| loaded))
}

impl SteamLibrary {
    fn load() -> Result<Self, SteamError> {
        let path = discover_steamclient_path()?;
        // SAFETY: `steamclient`'s initialisers spawn auxiliary threads
        // that hold code-pointers into the library's text segment, so
        // the handle is parked in the `STEAM_LIBRARY` `OnceLock` for
        // process lifetime — unloading would unmap the text segment and
        // crash those threads.
        let handle = unsafe { load_steamclient(&path) }
            .map_err(|source| SteamError::LibraryLoadFailed { path, source })?;

        // SAFETY: Both symbols are NUL-terminated byte-string literals.
        // `BGetCallbackFn` and `FreeLastCallbackFn` match the exported C
        // signatures exactly. The function pointers are transmuted out of
        // the `Symbol` wrapper (which borrows `handle`); this is safe
        // because `handle` is stored in the same `SteamLibrary` struct,
        // so the text-segment backing the pointers lives at least as long
        // as the pointers themselves.  `SteamLibrary` is parked in a
        // process-lifetime `OnceLock`, so the pointers are effectively
        // `'static`.  They may be called from any thread that holds a
        // valid `HSteamPipe` — dlopen'd ELF code-pointers are
        // thread-safe to call concurrently (they do not require
        // synchronisation on their own code-path; Steam's internal locks
        // protect shared state inside the library).
        tracing::info!(target: "loader", "loader: resolving Steam_BGetCallback");
        let b_get_callback_fn: BGetCallbackFn = unsafe {
            let sym: Symbol<BGetCallbackFn> =
                handle.get(b"Steam_BGetCallback\0").map_err(|source| {
                    SteamError::SymbolNotFound {
                        symbol: "Steam_BGetCallback",
                        source,
                    }
                })?;
            tracing::info!(target: "loader", "loader: resolved Steam_BGetCallback");
            let fn_ptr = *sym;
            tracing::info!(target: "loader", "loader: copied Steam_BGetCallback fn ptr");
            fn_ptr
        };
        tracing::info!(target: "loader", "loader: resolving Steam_FreeLastCallback");
        let free_last_callback_fn: FreeLastCallbackFn = unsafe {
            let sym: Symbol<FreeLastCallbackFn> =
                handle.get(b"Steam_FreeLastCallback\0").map_err(|source| {
                    SteamError::SymbolNotFound {
                        symbol: "Steam_FreeLastCallback",
                        source,
                    }
                })?;
            tracing::info!(target: "loader", "loader: resolved Steam_FreeLastCallback");
            let fn_ptr = *sym;
            tracing::info!(target: "loader", "loader: copied Steam_FreeLastCallback fn ptr");
            fn_ptr
        };

        tracing::info!(target: "loader", "loader: SteamLibrary constructed");
        Ok(Self {
            handle,
            b_get_callback_fn,
            free_last_callback_fn,
        })
    }

    pub fn b_get_callback(
        &self,
        pipe: HSteamPipe,
        msg: *mut CallbackMessage,
        call_handle: *mut i32,
    ) -> Result<bool, SteamError> {
        // SAFETY: `b_get_callback_fn` was resolved from the same
        // `handle` that this `SteamLibrary` owns; the handle lives for
        // process lifetime (stored in `STEAM_LIBRARY`).  `pipe` is a
        // live Steam pipe handle; Steam writes through `msg` only when
        // the return value is `true`; `call_handle` may be null.
        Ok(unsafe { (self.b_get_callback_fn)(pipe, msg, call_handle) })
    }

    pub fn free_last_callback(&self, pipe: HSteamPipe) -> Result<(), SteamError> {
        // SAFETY: `free_last_callback_fn` was resolved from the same
        // `handle` that this `SteamLibrary` owns; the handle lives for
        // process lifetime.  This is called exactly once per successful
        // `b_get_callback` return, before any further pipe use.
        unsafe { (self.free_last_callback_fn)(pipe) };
        Ok(())
    }

    pub fn create_interface(&self, version: &str) -> Result<RawInterface, SteamError> {
        let symbol_name = b"CreateInterface\0";
        // SAFETY: NUL-terminated symbol; `CreateInterfaceFn` matches
        // `void* CreateInterface(const char*, int*)`.
        let create: Symbol<CreateInterfaceFn> = unsafe {
            self.handle
                .get(symbol_name)
                .map_err(|source| SteamError::SymbolNotFound {
                    symbol: "CreateInterface",
                    source,
                })?
        };
        let c_version = CString::new(version).map_err(|_| SteamError::InvalidInterfaceVersion {
            version: version.to_owned(),
        })?;
        // SAFETY: `c_version` is NUL-terminated and outlives the call;
        // null `return_code` skips the write-back path.
        let raw = unsafe { create(c_version.as_ptr(), core::ptr::null_mut()) };
        if raw.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: version.to_owned(),
            });
        }
        Ok(raw)
    }
}

unsafe fn load_steamclient(path: &Path) -> Result<Library, libloading::Error> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;

        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_USER_DIRS, Library as WinLibrary,
        };
        use windows_sys::Win32::System::LibraryLoader::{
            AddDllDirectory, SetDefaultDllDirectories,
        };

        // SAFETY: standard Win32 call; no pointer arguments. Sets the
        // process-wide default search order to the modern flag set required
        // by `AddDllDirectory`.
        let default_ok =
            unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS as u32) != 0 };
        if !default_ok {
            // SAFETY: `GetLastError` is always safe immediately after a
            // failing Win32 call on the same thread.
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            tracing::warn!(
                last_error = err,
                "loader: SetDefaultDllDirectories failed (non-fatal)"
            );
        }

        if let Some(steam_root) = path.parent() {
            for dir in [steam_root.to_path_buf(), steam_root.join("bin")] {
                let wide: Vec<u16> = dir
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0u16))
                    .collect();
                // SAFETY: `wide` is a valid NUL-terminated UTF-16 string we
                // own on the stack. `AddDllDirectory` reads but does not
                // retain the pointer after returning. The returned cookie is
                // discarded — we never remove the directory.
                let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
                if !cookie.is_null() {
                    tracing::info!(dir = %dir.display(), "loader: AddDllDirectory ok");
                } else {
                    // SAFETY: see above.
                    let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                    tracing::warn!(
                        dir = %dir.display(),
                        last_error = err,
                        "loader: AddDllDirectory failed (non-fatal)"
                    );
                }
            }
        }

        tracing::info!(
            path = %path.display(),
            "loader: LoadLibraryExW (LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_USER_DIRS)"
        );
        // SAFETY: forwarded from the caller's unsafe contract; `path` is a
        // valid filesystem path to steamclient64.dll. The combined flags
        // make the loader search the application dir, System32, and every
        // directory previously registered via `AddDllDirectory` — i.e.
        // both Steam root and Steam\bin.
        let flags = LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_USER_DIRS;
        let result = unsafe { WinLibrary::load_with_flags(path, flags).map(Library::from) };
        match &result {
            Ok(_) => tracing::info!("loader: LoadLibraryExW succeeded"),
            Err(e) => tracing::error!(error = %e, "loader: LoadLibraryExW failed"),
        }
        return result;
    }
    #[cfg(not(target_os = "windows"))]
    // SAFETY: forwarded from the caller's unsafe contract.
    unsafe {
        Library::new(path)
    }
}

fn discover_steamclient_path() -> Result<PathBuf, SteamError> {
    let candidates = paths::steamclient_lib_candidates();
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }
    Err(SteamError::SteamInstallNotFound {
        searched: candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a real Steam install or STEAMLENS_STEAM_ROOT env override"]
    fn steamclient_loads_smoke() {
        SteamLibrary::load().expect("steamclient must open");
    }
}
