use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use image::{ColorType, ImageReader};
use steamlens_core::AppLibraryAssets;
use tokio::fs;

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

fn http_client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            match reqwest::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_REQUEST_TIMEOUT)
                .build()
            {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "reqwest::Client init failed; capsule HTTP fetches disabled"
                    );
                    None
                }
            }
        })
        .as_ref()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CapsuleSize {
    Small,
    #[default]
    Medium,
    Large,
    Portrait,
}

impl std::fmt::Display for CapsuleSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleSize::Small => write!(f, "Small"),
            CapsuleSize::Medium => write!(f, "Medium"),
            CapsuleSize::Large => write!(f, "Large"),
            CapsuleSize::Portrait => write!(f, "Portrait"),
        }
    }
}

fn size_suffix(size: CapsuleSize) -> &'static str {
    match size {
        CapsuleSize::Small => "small",
        CapsuleSize::Medium => "medium",
        CapsuleSize::Large => "large",
        CapsuleSize::Portrait => "portrait",
    }
}

fn asset_for_size(size: CapsuleSize, assets: &AppLibraryAssets) -> Option<(&'static str, &str)> {
    match size {
        CapsuleSize::Portrait => assets
            .library_capsule
            .as_deref()
            .map(|h| ("library_capsule.jpg", h)),
        CapsuleSize::Small | CapsuleSize::Medium | CapsuleSize::Large => assets
            .library_header
            .as_deref()
            .map(|h| ("library_header.jpg", h)),
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
            CapsuleError::NotFound => write!(f, "capsule not found (no hash or 404)"),
            CapsuleError::Decode(e) => write!(f, "decode error: {e}"),
            CapsuleError::Http(e) => write!(f, "http error: {e}"),
        }
    }
}

fn cache_filename(app_id: u32, size: CapsuleSize, hash: &str) -> String {
    format!("{app_id}_{}_{hash}.jpg", size_suffix(size))
}

fn cache_path(app_id: u32, size: CapsuleSize, hash: &str) -> PathBuf {
    crate::paths::capsules_dir().join(cache_filename(app_id, size, hash))
}

async fn purge_stale_caches(app_id: u32, size: CapsuleSize, keep_filename: &str) {
    let dir = crate::paths::capsules_dir();
    let prefix = format!("{app_id}_{}_", size_suffix(size));
    let Ok(mut entries) = fs::read_dir(&dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) && name_str.as_ref() != keep_filename {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
}

struct DecodedCapsule {
    pixels: CapsulePixels,
    is_placeholder: bool,
}

fn decode_jpeg(bytes: &[u8]) -> Result<DecodedCapsule, CapsuleError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| CapsuleError::Decode(e.to_string()))?;
    let img = reader
        .decode()
        .map_err(|e| CapsuleError::Decode(e.to_string()))?;
    let is_placeholder = matches!(img.color(), ColorType::L8 | ColorType::La8);
    let img = img.to_rgba8();
    let width = img.width();
    let height = img.height();
    Ok(DecodedCapsule {
        pixels: CapsulePixels {
            rgba: img.into_raw(),
            width,
            height,
        },
        is_placeholder,
    })
}

pub async fn fetch_capsule(
    app_id: u32,
    size: CapsuleSize,
    assets: AppLibraryAssets,
) -> Result<(CapsuleSize, CapsulePixels), (CapsuleSize, CapsuleError)> {
    let Some((filename, hash)) = asset_for_size(size, &assets) else {
        return Err((size, CapsuleError::NotFound));
    };

    let target_filename = cache_filename(app_id, size, hash);
    let path = cache_path(app_id, size, hash);

    if let Ok(bytes) = fs::read(&path).await {
        match decode_jpeg(&bytes) {
            Ok(decoded) if !decoded.is_placeholder => {
                return Ok((size, decoded.pixels));
            }
            _ => {
                let _ = fs::remove_file(&path).await;
            }
        }
    }

    let Some(client) = http_client() else {
        return Err((
            size,
            CapsuleError::Http("HTTP client unavailable".to_owned()),
        ));
    };

    let url = format!(
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{hash}/{filename}"
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err((size, CapsuleError::NotFound));
            }
            if !response.status().is_success() {
                return Err((
                    size,
                    CapsuleError::Http(format!("HTTP {}", response.status().as_u16())),
                ));
            }
            match response.bytes().await {
                Ok(bytes) => match decode_jpeg(&bytes) {
                    Ok(decoded) if !decoded.is_placeholder => {
                        if let Some(parent) = path.parent() {
                            let _ = fs::create_dir_all(parent).await;
                        }
                        let _ = fs::write(&path, &bytes).await;
                        purge_stale_caches(app_id, size, &target_filename).await;
                        Ok((size, decoded.pixels))
                    }
                    Ok(_) => {
                        tracing::trace!(
                            target: "capsule",
                            app_id,
                            filename,
                            "hashed CDN returned a grayscale placeholder; treating as unavailable"
                        );
                        Err((size, CapsuleError::NotFound))
                    }
                    Err(e) => Err((size, e)),
                },
                Err(e) => Err((size, CapsuleError::Http(e.to_string()))),
            }
        }
        Err(e) => Err((size, CapsuleError::Http(e.to_string()))),
    }
}
