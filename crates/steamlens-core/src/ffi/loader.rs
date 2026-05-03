use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libloading::{Library, Symbol};

use crate::error::SteamError;
use crate::ffi::interfaces::{
    BGetCallbackFn, CallbackMessage, CreateInterfaceFn, FreeLastCallbackFn, HSteamPipe,
};
use crate::ffi::opaque::RawInterface;

pub struct SteamLibrary {
    handle: Library,
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
        // SAFETY: `Library::new` runs the loaded library's initializers.
        // `steamclient.so` is a well-behaved shared object shipped by Valve;
        // its initializers do not invoke arbitrary user code in this
        // process. The path was resolved by `discover_steamclient_path`
        // which only accepts paths inside the user's Steam install.
        //
        // The library MUST live for the entire process lifetime.
        // `steamclient.so`'s ELF initialisers spawn auxiliary pthreads
        // (callback dispatch, IPC reader) that retain code-pointers into
        // this library's text segment for the process lifetime; unloading
        // it via `dlclose` unmaps that text segment and crashes those
        // threads on their next instruction. The `SteamLibrary` value is
        // therefore owned by the global `STEAM_LIBRARY` `OnceLock` above
        // and intentionally leaked — `Drop` for `Library` (which calls
        // `dlclose`) must never run.
        let handle = unsafe { Library::new(&path) }
            .map_err(|source| SteamError::LibraryLoadFailed { path, source })?;
        Ok(Self { handle })
    }

    pub fn b_get_callback(
        &self,
        pipe: HSteamPipe,
        msg: *mut CallbackMessage,
        call_handle: *mut i32,
    ) -> Result<bool, SteamError> {
        let symbol_name = b"Steam_BGetCallback\0";
        // SAFETY: `Steam_BGetCallback` is a documented C export of
        // `steamclient.so`. Its signature is
        // `bool Steam_BGetCallback(HSteamPipe, CallbackMessage*, int*)`.
        // The symbol name is NUL-terminated. We type-erase it through the
        // `BGetCallbackFn` alias which matches the export exactly.
        let func: Symbol<BGetCallbackFn> = unsafe {
            self.handle
                .get(symbol_name)
                .map_err(|source| SteamError::SymbolNotFound {
                    symbol: "Steam_BGetCallback",
                    source,
                })?
        };
        // SAFETY: `pipe` is a valid handle returned by `CreateSteamPipe`.
        // `msg` points to a `CallbackMessage` that lives on the caller's
        // stack and remains valid for the duration of this call. Steam
        // writes through `msg` only when it returns `true`; we copy the
        // payload before calling `free_last_callback`. `call_handle` may
        // be null — Steam skips writing back when it is.
        Ok(unsafe { func(pipe, msg, call_handle) })
    }

    pub fn free_last_callback(&self, pipe: HSteamPipe) -> Result<(), SteamError> {
        let symbol_name = b"Steam_FreeLastCallback\0";
        // SAFETY: `Steam_FreeLastCallback` is a documented C export of
        // `steamclient.so` whose signature is `bool Steam_FreeLastCallback(HSteamPipe)`.
        // The symbol name is NUL-terminated and typed via `FreeLastCallbackFn`.
        let func: Symbol<FreeLastCallbackFn> = unsafe {
            self.handle
                .get(symbol_name)
                .map_err(|source| SteamError::SymbolNotFound {
                    symbol: "Steam_FreeLastCallback",
                    source,
                })?
        };
        // SAFETY: `pipe` is the same valid handle used in the preceding
        // `b_get_callback` call. Steam_FreeLastCallback must be called once
        // per successful `b_get_callback`; we call it immediately after
        // copying the payload, before any further use of the pipe.
        unsafe { func(pipe) };
        Ok(())
    }

    pub fn create_interface(&self, version: &str) -> Result<RawInterface, SteamError> {
        let symbol_name = b"CreateInterface\0";
        // SAFETY: `CreateInterface` is a documented C export of every
        // shipped Steam client library. Its signature
        // `void* CreateInterface(const char* version, int* return_code)`
        // is stable across Steam updates. We type the symbol as
        // `CreateInterfaceFn` which matches that signature exactly.
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
        // SAFETY: `c_version` is a NUL-terminated UTF-8 string and outlives
        // this call. Passing a null `return_code` is the standard pattern;
        // Steam skips writing back when the pointer is null.
        let raw = unsafe { create(c_version.as_ptr(), core::ptr::null_mut()) };
        if raw.is_null() {
            return Err(SteamError::InterfaceUnavailable {
                version: version.to_owned(),
            });
        }
        Ok(raw)
    }
}

fn discover_steamclient_path() -> Result<PathBuf, SteamError> {
    let candidates = candidate_paths();
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }
    Err(SteamError::SteamInstallNotFound {
        searched: candidates,
    })
}

fn candidate_paths() -> Vec<PathBuf> {
    candidate_paths_from_env(env::var_os("HOME"), env::var_os("XDG_DATA_HOME"))
}

fn candidate_paths_from_env(
    home: Option<std::ffi::OsString>,
    xdg_data_home: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(3);

    if let Some(ref home) = home {
        out.push(Path::new(home).join(".steam/steam/linux64/steamclient.so"));
    }

    if let Some(ref xdg) = xdg_data_home {
        out.push(Path::new(xdg).join("Steam/linux64/steamclient.so"));
    }

    if let Some(ref home) = home {
        out.push(Path::new(home).join(".local/share/Steam/linux64/steamclient.so"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::candidate_paths_from_env;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn returns_three_paths_when_home_and_xdg_present() {
        let paths = candidate_paths_from_env(
            Some(OsString::from("/home/alice")),
            Some(OsString::from("/home/alice/.local/share")),
        );
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/alice/.steam/steam/linux64/steamclient.so"),
                PathBuf::from("/home/alice/.local/share/Steam/linux64/steamclient.so"),
                PathBuf::from("/home/alice/.local/share/Steam/linux64/steamclient.so"),
            ]
        );
    }

    #[test]
    fn skips_xdg_when_unset() {
        let paths = candidate_paths_from_env(Some(OsString::from("/home/bob")), None);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/bob/.steam/steam/linux64/steamclient.so"),
                PathBuf::from("/home/bob/.local/share/Steam/linux64/steamclient.so"),
            ]
        );
    }

    #[test]
    fn skips_home_dependent_entries_when_home_unset() {
        let paths = candidate_paths_from_env(None, Some(OsString::from("/srv/steam")));
        assert_eq!(
            paths,
            vec![PathBuf::from("/srv/steam/Steam/linux64/steamclient.so")]
        );
    }

    #[test]
    fn returns_empty_when_neither_var_present() {
        let paths = candidate_paths_from_env(None, None);
        assert!(paths.is_empty());
    }

    #[test]
    fn canonical_steam_symlink_is_probed_first() {
        let paths = candidate_paths_from_env(
            Some(OsString::from("/home/carol")),
            Some(OsString::from("/home/carol/.local/share")),
        );
        assert_eq!(
            paths.first().unwrap(),
            &PathBuf::from("/home/carol/.steam/steam/linux64/steamclient.so")
        );
    }
}
