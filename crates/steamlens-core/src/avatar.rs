use std::path::Path;

use crate::Image;
use crate::error::SteamError;
use crate::paths;

#[derive(Debug, thiserror::Error)]
pub enum AvatarLoadError {
    #[error("Steam install root not found")]
    SteamRootNotFound,
    #[error("Avatar PNG not found in any candidate Steam install (steamid64={steamid64})")]
    NotFound { steamid64: u64 },
    #[error("Failed to read avatar PNG at {path}: {source}", path = .path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to decode avatar PNG at {path}: {source}", path = .path.display())]
    Decode {
        path: std::path::PathBuf,
        #[source]
        source: image::ImageError,
    },
}

impl From<AvatarLoadError> for SteamError {
    fn from(err: AvatarLoadError) -> Self {
        SteamError::AvatarCacheFailed {
            message: err.to_string(),
        }
    }
}

pub fn read_local_avatar(steamid64: u64) -> Result<Image, AvatarLoadError> {
    let candidates = paths::steam_install_root_candidates();
    if candidates.is_empty() {
        return Err(AvatarLoadError::SteamRootNotFound);
    }
    for root in &candidates {
        let path = paths::avatar_cache_path(root, steamid64);
        if path.exists() {
            return decode_png(&path);
        }
    }
    Err(AvatarLoadError::NotFound { steamid64 })
}

pub fn read_avatar_from_root(steam_root: &Path, steamid64: u64) -> Result<Image, AvatarLoadError> {
    let path = paths::avatar_cache_path(steam_root, steamid64);
    if !path.exists() {
        return Err(AvatarLoadError::NotFound { steamid64 });
    }
    decode_png(&path)
}

fn decode_png(path: &Path) -> Result<Image, AvatarLoadError> {
    let bytes = std::fs::read(path).map_err(|source| AvatarLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let img = image::load_from_memory(&bytes).map_err(|source| AvatarLoadError::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Image {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn write_synthetic_png(path: &Path, w: u32, h: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let img = image::RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([(x & 0xFF) as u8, (y & 0xFF) as u8, 0, 255])
        });
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        std::fs::write(path, buf.into_inner()).unwrap();
    }

    #[test]
    fn read_avatar_from_root_returns_decoded_image() {
        let tmp = TempDir::new().unwrap();
        let steamid64 = 76561198000000001;
        write_synthetic_png(&paths::avatar_cache_path(tmp.path(), steamid64), 64, 64);

        let img = read_avatar_from_root(tmp.path(), steamid64).expect("decoded");
        assert_eq!(img.width, 64);
        assert_eq!(img.height, 64);
        assert_eq!(img.rgba.len(), 64 * 64 * 4);
    }

    #[test]
    fn read_avatar_from_root_reports_not_found() {
        let tmp = TempDir::new().unwrap();
        let err = read_avatar_from_root(tmp.path(), 76561198000000002).unwrap_err();
        assert!(matches!(err, AvatarLoadError::NotFound { .. }));
    }

    #[test]
    fn read_avatar_from_root_reports_decode_error_on_garbage() {
        let tmp = TempDir::new().unwrap();
        let steamid64 = 76561198000000003;
        let path = paths::avatar_cache_path(tmp.path(), steamid64);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not a png").unwrap();

        let err = read_avatar_from_root(tmp.path(), steamid64).unwrap_err();
        assert!(matches!(err, AvatarLoadError::Decode { .. }));
    }
}
