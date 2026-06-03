use std::env;
use std::path::{Path, PathBuf};

pub fn steam_install_root_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    return steam_roots_linux(env::var_os("HOME"), env::var_os("XDG_DATA_HOME"));

    #[cfg(target_os = "macos")]
    return steam_roots_macos(env::var_os("HOME"));

    #[cfg(target_os = "windows")]
    return steam_roots_windows();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Vec::new()
}

pub fn steamclient_lib_candidates() -> Vec<PathBuf> {
    let subpath = steamclient_lib_subpath();
    steam_install_root_candidates()
        .into_iter()
        .map(|root| {
            let mut p = root;
            for segment in &subpath {
                p = p.join(segment);
            }
            p
        })
        .collect()
}

pub fn user_data_dir(steam_root: &Path, steamid3: u32) -> PathBuf {
    steam_root.join("userdata").join(steamid3.to_string())
}

pub fn appcache_stats_dir(steam_root: &Path) -> PathBuf {
    steam_root.join("appcache").join("stats")
}

fn steamclient_lib_subpath() -> Vec<&'static str> {
    #[cfg(target_os = "linux")]
    return vec!["linux64", "steamclient.so"];
    #[cfg(target_os = "macos")]
    return vec![
        "Steam.AppBundle",
        "Steam",
        "Contents",
        "MacOS",
        "steamclient.dylib",
    ];
    #[cfg(target_os = "windows")]
    return vec!["steamclient64.dll"];
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return vec![];
}

#[cfg(target_os = "linux")]
fn steam_roots_linux(
    home: Option<std::ffi::OsString>,
    xdg_data_home: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(3);
    if let Some(ref h) = home {
        out.push(PathBuf::from(h).join(".steam").join("steam"));
    }
    if let Some(ref xdg) = xdg_data_home {
        out.push(PathBuf::from(xdg).join("Steam"));
    }
    if let Some(ref h) = home {
        out.push(PathBuf::from(h).join(".local").join("share").join("Steam"));
    }
    out
}

#[cfg(target_os = "macos")]
fn steam_roots_macos(home: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(1);
    if let Some(ref h) = home {
        out.push(
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("Steam"),
        );
    }
    out
}

#[cfg(target_os = "windows")]
fn steam_roots_windows() -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(3);
    if let Some(reg) = read_steam_root_from_registry() {
        out.push(reg);
    }
    if let Ok(pf86) = env::var("ProgramFiles(x86)") {
        out.push(PathBuf::from(pf86).join("Steam"));
    }
    if let Ok(pf) = env::var("ProgramFiles") {
        out.push(PathBuf::from(pf).join("Steam"));
    }
    out
}

#[cfg(target_os = "windows")]
fn read_steam_root_from_registry() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam")
        && let Ok(path) = key.get_value::<String, _>("SteamPath")
    {
        return Some(PathBuf::from(path.replace('/', "\\")));
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        && let Ok(path) = key.get_value::<String, _>("InstallPath")
    {
        return Some(PathBuf::from(path.replace('/', "\\")));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_roots_three_when_home_and_xdg() {
        let roots = steam_roots_linux(
            Some(OsString::from("/home/alice")),
            Some(OsString::from("/home/alice/.local/share")),
        );
        assert_eq!(roots.len(), 3);
        assert_eq!(roots[0], PathBuf::from("/home/alice/.steam/steam"));
        assert_eq!(roots[1], PathBuf::from("/home/alice/.local/share/Steam"));
        assert_eq!(roots[2], PathBuf::from("/home/alice/.local/share/Steam"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_roots_two_without_xdg() {
        let roots = steam_roots_linux(Some(OsString::from("/home/bob")), None);
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0], PathBuf::from("/home/bob/.steam/steam"));
        assert_eq!(roots[1], PathBuf::from("/home/bob/.local/share/Steam"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_roots_empty_without_home_or_xdg() {
        assert!(steam_roots_linux(None, None).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn steamclient_candidates_append_lib_subpath() {
        let candidates = steamclient_lib_candidates();
        for c in &candidates {
            assert!(c.to_string_lossy().ends_with("linux64/steamclient.so"));
        }
    }
}
