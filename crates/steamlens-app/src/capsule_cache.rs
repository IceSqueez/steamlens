use std::io::Cursor;
use std::path::PathBuf;

use image::ImageReader;
use tokio::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CapsuleSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl std::fmt::Display for CapsuleSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleSize::Small => write!(f, "Small"),
            CapsuleSize::Medium => write!(f, "Medium"),
            CapsuleSize::Large => write!(f, "Large"),
        }
    }
}

fn fallback_chain(size: CapsuleSize) -> &'static [&'static str] {
    match size {
        CapsuleSize::Small => &[
            "capsule_sm_120.jpg",
            "capsule_231x87.jpg",
            "header.jpg",
            "library_hero.jpg",
        ],
        CapsuleSize::Medium => &[
            "capsule_231x87.jpg",
            "capsule_sm_120.jpg",
            "header.jpg",
            "library_hero.jpg",
        ],
        CapsuleSize::Large => &[
            "header.jpg",
            "library_hero.jpg",
            "capsule_231x87.jpg",
            "capsule_sm_120.jpg",
        ],
    }
}

fn size_suffix(size: CapsuleSize) -> &'static str {
    match size {
        CapsuleSize::Small => "small",
        CapsuleSize::Medium => "medium",
        CapsuleSize::Large => "large",
    }
}

#[derive(Debug)]
pub struct CapsulePixels {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum CapsuleError {
    NotFound,
    Decode(String),
    Http(String),
}

impl std::fmt::Display for CapsuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleError::NotFound => write!(f, "capsule not found (404 or unavailable)"),
            CapsuleError::Decode(e) => write!(f, "decode error: {e}"),
            CapsuleError::Http(e) => write!(f, "http error: {e}"),
        }
    }
}

pub fn cache_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs_base().join("Library/Caches/steamlens/capsules")
    }
    #[cfg(windows)]
    {
        dirs_base().join("steamlens\\capsules")
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        dirs_base().join("steamlens/capsules")
    }
}

fn dirs_base() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs_home()
    }
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_home())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_home().join(".cache"))
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn cache_path(app_id: u32, size: CapsuleSize) -> PathBuf {
    cache_dir().join(format!("{app_id}_{}.jpg", size_suffix(size)))
}

fn decode_jpeg(bytes: &[u8]) -> Result<CapsulePixels, CapsuleError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| CapsuleError::Decode(e.to_string()))?;
    let img = reader
        .decode()
        .map_err(|e| CapsuleError::Decode(e.to_string()))?
        .to_rgba8();
    let width = img.width();
    let height = img.height();
    Ok(CapsulePixels {
        rgba: img.into_raw(),
        width,
        height,
    })
}

pub async fn fetch_capsule(
    app_id: u32,
    size: CapsuleSize,
) -> Result<(CapsuleSize, CapsulePixels), (CapsuleSize, CapsuleError)> {
    let path = cache_path(app_id, size);

    if let Ok(bytes) = fs::read(&path).await {
        return decode_jpeg(&bytes)
            .map(|p| (size, p))
            .map_err(|e| (size, e));
    }

    let mut last_err = CapsuleError::NotFound;
    for filename in fallback_chain(size) {
        let url = format!(
            "https://shared.steamstatic.com/store_item_assets/steam/apps/{app_id}/{filename}"
        );
        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    last_err = CapsuleError::NotFound;
                    continue;
                }
                if !response.status().is_success() {
                    last_err = CapsuleError::Http(format!("HTTP {}", response.status().as_u16()));
                    continue;
                }
                match response.bytes().await {
                    Ok(bytes) => {
                        if let Some(parent) = path.parent() {
                            let _ = fs::create_dir_all(parent).await;
                        }
                        let _ = fs::write(&path, &bytes).await;
                        return decode_jpeg(&bytes)
                            .map(|p| (size, p))
                            .map_err(|e| (size, e));
                    }
                    Err(e) => {
                        last_err = CapsuleError::Http(e.to_string());
                        continue;
                    }
                }
            }
            Err(e) => {
                last_err = CapsuleError::Http(e.to_string());
                continue;
            }
        }
    }

    Err((size, last_err))
}
