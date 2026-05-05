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
        // SAFETY: `steamclient.so`'s ELF initialisers spawn auxiliary
        // pthreads that hold code-pointers into the library's text
        // segment, so the handle is parked in the `STEAM_LIBRARY`
        // `OnceLock` for process lifetime — `dlclose` would unmap the
        // text segment and crash those threads.
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
        // SAFETY: NUL-terminated symbol; `BGetCallbackFn` matches the
        // exported `bool Steam_BGetCallback(HSteamPipe, CallbackMessage*, int*)`.
        let func: Symbol<BGetCallbackFn> = unsafe {
            self.handle
                .get(symbol_name)
                .map_err(|source| SteamError::SymbolNotFound {
                    symbol: "Steam_BGetCallback",
                    source,
                })?
        };
        // SAFETY: live pipe; Steam writes through `msg` only on `true`
        // return; `call_handle` may be null.
        Ok(unsafe { func(pipe, msg, call_handle) })
    }

    pub fn free_last_callback(&self, pipe: HSteamPipe) -> Result<(), SteamError> {
        let symbol_name = b"Steam_FreeLastCallback\0";
        // SAFETY: NUL-terminated symbol; `FreeLastCallbackFn` matches
        // `bool Steam_FreeLastCallback(HSteamPipe)`.
        let func: Symbol<FreeLastCallbackFn> = unsafe {
            self.handle
                .get(symbol_name)
                .map_err(|source| SteamError::SymbolNotFound {
                    symbol: "Steam_FreeLastCallback",
                    source,
                })?
        };
        // SAFETY: called exactly once per successful `b_get_callback`,
        // before any further pipe use.
        unsafe { func(pipe) };
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
