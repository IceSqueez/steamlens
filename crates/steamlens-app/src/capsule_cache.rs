use std::io::Cursor;
use std::path::PathBuf;

use image::ImageReader;
use tokio::fs;

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

fn cache_path(app_id: u32) -> PathBuf {
    cache_dir().join(format!("{app_id}.jpg"))
}

fn capsule_url(app_id: u32) -> String {
    format!(
        "https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{app_id}/capsule_sm_120.jpg"
    )
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

pub async fn fetch_capsule(app_id: u32) -> Result<CapsulePixels, CapsuleError> {
    let path = cache_path(app_id);

    if let Ok(bytes) = fs::read(&path).await {
        return decode_jpeg(&bytes);
    }

    let url = capsule_url(app_id);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| CapsuleError::Http(e.to_string()))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(CapsuleError::NotFound);
    }

    if !response.status().is_success() {
        return Err(CapsuleError::Http(format!(
            "HTTP {}",
            response.status().as_u16()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| CapsuleError::Http(e.to_string()))?;

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(&path, &bytes).await;

    decode_jpeg(&bytes)
}
