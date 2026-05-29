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
    let mut chain = Vec::with_capacity(4);
    match size {
        CapsuleSize::Portrait => {
            if let Some(a) = assets.cover.as_ref() {
                chain.push(a);
            }
        }
        CapsuleSize::Small | CapsuleSize::Medium | CapsuleSize::Large => {
            if let Some(a) = assets.wide_cover.as_ref() {
                chain.push(a);
            }
            if let Some(a) = assets.wide_cover_legacy.as_ref() {
                chain.push(a);
            }
            if let Some(a) = assets.logo.as_ref() {
                chain.push(a);
            }
            if let Some(a) = assets.background.as_ref() {
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

pub async fn purge_for_app(app_id: u32) {
    let dir = crate::paths::capsules_dir();
    let prefix = format!("{app_id}_");
    let Ok(mut entries) = fs::read_dir(&dir).await else {
        return;
    };
    let mut deleted = 0u32;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(&prefix)
            && fs::remove_file(entry.path()).await.is_ok()
        {
            deleted += 1;
        }
    }
    tracing::debug!(app_id, deleted, "purged all capsule cache files for app");
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

fn decode_jpeg_sync(bytes: &[u8]) -> Option<DecodedCapsule> {
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

async fn decode_jpeg_async(bytes: Vec<u8>) -> Option<DecodedCapsule> {
    tokio::task::spawn_blocking(move || decode_jpeg_sync(&bytes))
        .await
        .ok()
        .flatten()
}

async fn try_cache_candidate(
    app_id: u32,
    size: CapsuleSize,
    asset: &ImageAsset,
) -> Option<CapsulePixels> {
    let cache_key = cdn_cache_key(asset);
    let path = cache_path(app_id, size, cache_key);

    let bytes = match fs::read(&path).await {
        Ok(b) => b,
        Err(_) => {
            tracing::trace!(app_id, %size, cache_key, "cache miss");
            return None;
        }
    };

    match decode_jpeg_async(bytes).await {
        Some(decoded) if !decoded.is_placeholder => {
            tracing::trace!(
                app_id, %size, cache_key,
                w = decoded.pixels.width, h = decoded.pixels.height,
                "cache hit"
            );
            Some(decoded.pixels)
        }
        Some(_) => {
            tracing::warn!(
                app_id, %size, cache_key,
                path = %path.display(),
                "cached file decoded as grayscale placeholder; removing"
            );
            let _ = fs::remove_file(&path).await;
            None
        }
        None => {
            tracing::warn!(
                app_id, %size, cache_key,
                path = %path.display(),
                "cached file failed to JPEG-decode; removing"
            );
            let _ = fs::remove_file(&path).await;
            None
        }
    }
}

async fn try_http_candidate(
    app_id: u32,
    size: CapsuleSize,
    asset: &ImageAsset,
) -> Option<CapsulePixels> {
    let cache_key = cdn_cache_key(asset);
    let target_filename = cache_filename(app_id, size, cache_key);
    let path = cache_path(app_id, size, cache_key);
    let url = cdn_url(app_id, asset);

    let client = http_client()?;
    let _permit = http_semaphore().acquire().await.ok()?;

    tracing::debug!(app_id, %size, %url, "HTTP fetch start");

    let mut attempt: u8 = 0;
    let response = loop {
        attempt += 1;

        let r = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt < 2 {
                    tracing::trace!(
                        app_id, %url, error = %e,
                        "transport error; retrying once after backoff"
                    );
                    tokio::time::sleep(HTTP_RETRY_BACKOFF).await;
                    continue;
                }
                tracing::warn!(
                    app_id, %url, error = %e,
                    "HTTP send failed after one retry"
                );
                return None;
            }
        };

        let status = r.status();
        if status.is_success() {
            break r;
        }

        let transient =
            status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
        if transient && attempt < 2 {
            tracing::trace!(
                app_id, %url, status = status.as_u16(),
                "transient HTTP status; retrying once after backoff"
            );
            tokio::time::sleep(HTTP_RETRY_BACKOFF).await;
            continue;
        }

        tracing::debug!(
            app_id, %url, status = status.as_u16(),
            "CDN non-success response; trying next candidate"
        );
        return None;
    };

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(

                app_id, %url, error = %e,
                "failed to read HTTP body bytes"
            );
            return None;
        }
    };

    let bytes_for_decode = bytes.to_vec();
    let decoded = match decode_jpeg_async(bytes_for_decode).await {
        Some(d) => d,
        None => {
            tracing::warn!(
                app_id, %url, bytes_len = bytes.len(),
                "downloaded bytes failed JPEG decode"
            );
            return None;
        }
    };

    if decoded.is_placeholder {
        tracing::debug!(

            app_id, %url,
            "CDN returned grayscale placeholder; trying next candidate"
        );
        return None;
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    if let Err(e) = fs::write(&path, &bytes).await {
        tracing::warn!(

            app_id, path = %path.display(), error = %e,
            "cache write failed; capsule will refetch next restart"
        );
    } else {
        tracing::trace!(

            app_id, %size, cache_key,
            "wrote capsule to cache"
        );
    }
    purge_stale_caches(app_id, size, &target_filename).await;

    tracing::debug!(

        app_id, %size, %url,
        w = decoded.pixels.width, h = decoded.pixels.height,
        "capsule resolved via HTTP"
    );
    Some(decoded.pixels)
}

pub async fn fetch_capsule(
    app_id: u32,
    size: CapsuleSize,
    assets: AppLibraryAssets,
) -> Result<(CapsuleSize, CapsulePixels), (CapsuleSize, CapsuleError)> {
    let chain = asset_chain_for_size(size, &assets);
    if chain.is_empty() {
        tracing::debug!(

            app_id, %size,
            "no asset candidates in appinfo; capsule unavailable"
        );
        return Err((size, CapsuleError::NotFound));
    }

    for asset in &chain {
        if let Some(pixels) = try_cache_candidate(app_id, size, asset).await {
            return Ok((size, pixels));
        }
    }

    tracing::debug!(

        app_id, %size, candidates = chain.len(),
        "cache empty for all candidates; starting HTTP chain"
    );

    for asset in &chain {
        if let Some(pixels) = try_http_candidate(app_id, size, asset).await {
            return Ok((size, pixels));
        }
    }

    tracing::warn!(

        app_id, %size, candidates = chain.len(),
        "all candidates exhausted; capsule unavailable"
    );
    Err((size, CapsuleError::NotFound))
}
