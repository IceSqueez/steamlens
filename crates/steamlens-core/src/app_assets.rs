use std::collections::HashMap;
use std::path::{Path, PathBuf};

use steamlens_vdf::{AppLibraryAssets, parse_appinfo_assets};

pub fn read_app_assets(steam_root: &Path) -> HashMap<u32, AppLibraryAssets> {
    let path = appinfo_path(steam_root);
    let Ok(bytes) = std::fs::read(&path) else {
        return HashMap::new();
    };
    parse_appinfo_assets(&bytes).unwrap_or_default()
}

fn appinfo_path(steam_root: &Path) -> PathBuf {
    steam_root.join("appcache").join("appinfo.vdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_app_assets_missing_file_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let map = read_app_assets(tmp.path());
        assert!(map.is_empty());
    }

    #[test]
    fn appinfo_path_joins_under_appcache() {
        let p = appinfo_path(Path::new("/opt/steam"));
        assert_eq!(p, PathBuf::from("/opt/steam/appcache/appinfo.vdf"));
    }
}
