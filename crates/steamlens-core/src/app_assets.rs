use std::collections::HashMap;
use std::path::{Path, PathBuf};

use steamlens_vdf::{AppLibraryAssets, parse_appinfo_assets};

use crate::paths::steam_install_root_candidates;

pub fn read_app_assets(steam_root: &Path) -> HashMap<u32, AppLibraryAssets> {
    let path = appinfo_path(steam_root);
    let Ok(bytes) = std::fs::read(&path) else {
        return HashMap::new();
    };
    parse_appinfo_assets(&bytes).unwrap_or_default()
}

pub fn discover_app_assets() -> HashMap<u32, AppLibraryAssets> {
    for root in steam_install_root_candidates() {
        if appinfo_path(&root).exists() {
            return read_app_assets(&root);
        }
    }
    HashMap::new()
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
}
