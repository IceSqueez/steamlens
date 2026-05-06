use std::env;
use std::ffi::CString;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::Path;
use std::path::PathBuf;
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
    if let Some(p) = env_override() {
        return vec![p];
    }

    #[cfg(target_os = "linux")]
    return candidate_paths_linux(env::var_os("HOME"), env::var_os("XDG_DATA_HOME"));

    #[cfg(target_os = "macos")]
    return candidate_paths_macos(env::var_os("HOME"));

    #[cfg(target_os = "windows")]
    return candidate_paths_windows();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Vec::new()
}

fn env_override() -> Option<PathBuf> {
    let root = env::var_os("STEAMLENS_STEAM_ROOT")?;
    Some(PathBuf::from(root).join(library_subpath()))
}

#[cfg(target_os = "linux")]
fn library_subpath() -> &'static str {
    "linux64/steamclient.so"
}

#[cfg(target_os = "macos")]
fn library_subpath() -> &'static str {
    "Steam.AppBundle/Steam/Contents/MacOS/steamclient.dylib"
}

#[cfg(target_os = "windows")]
fn library_subpath() -> &'static str {
    "steamclient64.dll"
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn library_subpath() -> &'static str {
    ""
}

#[cfg(target_os = "linux")]
fn candidate_paths_linux(
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

#[cfg(target_os = "macos")]
fn candidate_paths_macos(home: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(2);

    if let Some(ref home) = home {
        // Per RFC-005 §R11 — UNVERIFIED. Confirm via `ls` on a real macOS
        // install before alpha.4 ship.
        out.push(Path::new(home).join(
            "Library/Application Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/steamclient.dylib",
        ));
        out.push(Path::new(home).join("Library/Application Support/Steam/steamclient.dylib"));
    }

    out
}

#[cfg(target_os = "windows")]
fn candidate_paths_windows() -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(3);

    if let Some(install) = read_steam_install_dir_from_registry() {
        out.push(install.join("steamclient64.dll"));
    }

    if let Ok(pf86) = env::var("ProgramFiles(x86)") {
        out.push(PathBuf::from(pf86).join("Steam").join("steamclient64.dll"));
    }

    if let Ok(pf) = env::var("ProgramFiles") {
        out.push(PathBuf::from(pf).join("Steam").join("steamclient64.dll"));
    }

    out
}

#[cfg(target_os = "windows")]
fn read_steam_install_dir_from_registry() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam")
        && let Ok(path) = key.get_value::<String, _>("SteamPath")
    {
        return Some(PathBuf::from(path));
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        && let Ok(path) = key.get_value::<String, _>("InstallPath")
    {
        return Some(PathBuf::from(path));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_returns_three_paths_when_home_and_xdg_present() {
        let paths = candidate_paths_linux(
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_skips_xdg_when_unset() {
        let paths = candidate_paths_linux(Some(OsString::from("/home/bob")), None);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/bob/.steam/steam/linux64/steamclient.so"),
                PathBuf::from("/home/bob/.local/share/Steam/linux64/steamclient.so"),
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_skips_home_dependent_entries_when_home_unset() {
        let paths = candidate_paths_linux(None, Some(OsString::from("/srv/steam")));
        assert_eq!(
            paths,
            vec![PathBuf::from("/srv/steam/Steam/linux64/steamclient.so")]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_returns_empty_when_neither_var_present() {
        let paths = candidate_paths_linux(None, None);
        assert!(paths.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_canonical_steam_symlink_is_probed_first() {
        let paths = candidate_paths_linux(
            Some(OsString::from("/home/carol")),
            Some(OsString::from("/home/carol/.local/share")),
        );
        assert_eq!(
            paths.first().unwrap(),
            &PathBuf::from("/home/carol/.steam/steam/linux64/steamclient.so")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_returns_app_bundle_first() {
        let paths = candidate_paths_macos(Some(OsString::from("/Users/alice")));
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0],
            PathBuf::from(
                "/Users/alice/Library/Application Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/steamclient.dylib"
            )
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_returns_empty_without_home() {
        assert!(candidate_paths_macos(None).is_empty());
    }

    #[test]
    fn env_override_returns_constructed_path() {
        // SAFETY: tests run sequentially per crate; setting env in this test does
        // not race with concurrent tests reading the same var.
        unsafe {
            env::set_var("STEAMLENS_STEAM_ROOT", "/tmp/synthetic_steam");
        }
        let p = env_override().expect("override must produce a path");
        assert!(p.starts_with("/tmp/synthetic_steam"));
        assert!(p.to_string_lossy().ends_with(library_subpath()));
        unsafe {
            env::remove_var("STEAMLENS_STEAM_ROOT");
        }
    }

    #[test]
    #[ignore = "requires a real Steam install or STEAMLENS_STEAM_ROOT env override"]
    fn steamclient_loads_smoke() {
        SteamLibrary::load().expect("steamclient must open");
    }
}
