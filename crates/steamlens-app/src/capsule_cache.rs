use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use image::{ColorType, ImageReader};
use steamlens_core::{AppLibraryAssets, ImageAsset};
use tokio::fs;

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const MAX_CONCURRENT_HTTP_FETCHES: usize = 8;

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

fn http_semaphore() -> &'static tokio::sync::Semaphore {
    static SEM: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_HTTP_FETCHES))
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

fn asset_chain_for_size(size: CapsuleSize, assets: &AppLibraryAssets) -> Vec<&ImageAsset> {
    let mut chain = Vec::with_capacity(3);
    match size {
        CapsuleSize::Portrait => {
            if let Some(a) = assets.library_capsule.as_ref() {
                chain.push(a);
            }
        }
        CapsuleSize::Small | CapsuleSize::Medium | CapsuleSize::Large => {
            if let Some(a) = assets.library_header.as_ref() {
                chain.push(a);
            }
            if let Some(a) = assets.library_hero.as_ref() {
                chain.push(a);
            }
            if let Some(a) = assets.header_image_legacy.as_ref() {
                chain.push(a);
            }
        }
    }
    chain
}

fn cdn_url(app_id: u32, asset: &ImageAsset) -> String {
    match asset {
        ImageAsset::Hashed { hash, filename } => format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{hash}/{filename}"
        ),
        ImageAsset::Plain { filename } => format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{filename}"
        ),
    }
}

fn cdn_cache_key(asset: &ImageAsset) -> &str {
    match asset {
        ImageAsset::Hashed { hash, .. } => hash,
        ImageAsset::Plain { filename } => filename,
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
}

impl std::fmt::Display for CapsuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleError::NotFound => write!(f, "no candidate returned real artwork"),
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
    let prefix_new = format!("{app_id}_{}_", size_suffix(size));
    let legacy_name = format!("{app_id}_{}.jpg", size_suffix(size));
    let Ok(mut entries) = fs::read_dir(&dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let is_legacy = name_str.as_ref() == legacy_name;
        let is_stale_new = name_str.starts_with(&prefix_new) && name_str.as_ref() != keep_filename;
        if is_legacy || is_stale_new {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
}

struct DecodedCapsule {
    pixels: CapsulePixels,
    is_placeholder: bool,
}

fn decode_jpeg(bytes: &[u8]) -> Option<DecodedCapsule> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let is_placeholder = matches!(img.color(), ColorType::L8 | ColorType::La8);
    let img = img.to_rgba8();
    let width = img.width();
    let height = img.height();
    Some(DecodedCapsule {
        pixels: CapsulePixels {
            rgba: img.into_raw(),
            width,
            height,
        },
        is_placeholder,
    })
}

async fn try_candidate(
    app_id: u32,
    size: CapsuleSize,
    asset: &ImageAsset,
) -> Option<CapsulePixels> {
    let cache_key = cdn_cache_key(asset);
    let target_filename = cache_filename(app_id, size, cache_key);
    let path = cache_path(app_id, size, cache_key);

    if let Ok(bytes) = fs::read(&path).await {
        match decode_jpeg(&bytes) {
            Some(decoded) if !decoded.is_placeholder => return Some(decoded.pixels),
            _ => {
                let _ = fs::remove_file(&path).await;
            }
        }
    }

    let client = http_client()?;
    let url = cdn_url(app_id, asset);

    let _permit = http_semaphore().acquire().await.ok()?;

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            tokio::time::sleep(HTTP_RETRY_BACKOFF).await;
            client.get(&url).send().await.ok()?
        }
    };

    if !response.status().is_success() {
        tracing::trace!(
            target: "capsule",
            app_id,
            url = %url,
            status = response.status().as_u16(),
            "CDN non-success; trying next candidate"
        );
        return None;
    }

    let bytes = response.bytes().await.ok()?;
    let decoded = decode_jpeg(&bytes)?;
    if decoded.is_placeholder {
        tracing::trace!(
            target: "capsule",
            app_id,
            url = %url,
            "CDN returned a grayscale placeholder; trying next candidate"
        );
        return None;
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(&path, &bytes).await;
    purge_stale_caches(app_id, size, &target_filename).await;
    Some(decoded.pixels)
}

pub async fn fetch_capsule(
    app_id: u32,
    size: CapsuleSize,
    assets: AppLibraryAssets,
) -> Result<(CapsuleSize, CapsulePixels), (CapsuleSize, CapsuleError)> {
    let chain = asset_chain_for_size(size, &assets);
    if chain.is_empty() {
        return Err((size, CapsuleError::NotFound));
    }
    for asset in chain {
        if let Some(pixels) = try_candidate(app_id, size, asset).await {
            return Ok((size, pixels));
        }
    }
    Err((size, CapsuleError::NotFound))
}
