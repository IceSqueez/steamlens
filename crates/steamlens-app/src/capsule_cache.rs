use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use image::ImageReader;
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
        CapsuleSize::Portrait => &[
            "library_600x900_2x.jpg",
            "library_600x900.jpg",
            "header.jpg",
        ],
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

fn cache_path(app_id: u32, size: CapsuleSize) -> PathBuf {
    crate::paths::capsules_dir().join(format!("{app_id}_{}.jpg", size_suffix(size)))
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

    let Some(client) = http_client() else {
        return Err((
            size,
            CapsuleError::Http("HTTP client unavailable".to_owned()),
        ));
    };

    let mut last_err = CapsuleError::NotFound;
    for filename in fallback_chain(size) {
        let url = format!(
            "https://shared.steamstatic.com/store_item_assets/steam/apps/{app_id}/{filename}"
        );
        match client.get(&url).send().await {
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
