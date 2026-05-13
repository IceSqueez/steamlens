use std::path::PathBuf;
use std::time::Duration;

use steamlens_core::AchievementIcon;

const CDN_HOST: &str = "https://cdn.cloudflare.steamstatic.com";
const CDN_PATH_PREFIX: &str = "/steamcommunity/public/images/apps";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum IconFetchError {
    #[error("HTTP request failed for {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("CDN returned {status} for {url}")]
    Status { url: String, status: u16 },
    #[error("Image decode failed for {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: image::ImageError,
    },
    #[error("I/O error at {path}: {source}", path = .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn cdn_url(app_id: u32, filename: &str) -> String {
    format!("{CDN_HOST}{CDN_PATH_PREFIX}/{app_id}/{filename}")
}

fn cache_path(app_id: u32, filename: &str) -> PathBuf {
    let safe = filename.replace(['/', '\\', ':', '\0'], "_");
    crate::paths::cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("achievement_icons")
        .join(safe)
}

pub async fn load_or_fetch(app_id: u32, filename: &str) -> Result<AchievementIcon, IconFetchError> {
    let path = cache_path(app_id, filename);
    if let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(icon) = decode(&bytes, &path.display().to_string())
    {
        return Ok(icon);
    }

    let bytes = fetch_from_cdn(app_id, filename).await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| IconFetchError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
    }
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|source| IconFetchError::Io {
            path: path.clone(),
            source,
        })?;
    decode(&bytes, &cdn_url(app_id, filename))
}

async fn fetch_from_cdn(app_id: u32, filename: &str) -> Result<Vec<u8>, IconFetchError> {
    let url = cdn_url(app_id, filename);
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|source| IconFetchError::Http {
            url: url.clone(),
            source,
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(IconFetchError::Status {
            url,
            status: status.as_u16(),
        });
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|source| IconFetchError::Http { url, source })
}

fn decode(bytes: &[u8], source_hint: &str) -> Result<AchievementIcon, IconFetchError> {
    let img = image::load_from_memory(bytes).map_err(|source| IconFetchError::Decode {
        url: source_hint.to_owned(),
        source,
    })?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(AchievementIcon {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .expect("reqwest::Client init for CDN icons failed")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_sanitises_filename_separators() {
        let p = cache_path(123, "weird/../name\\with:bad\0chars.jpg");
        let name = p.file_name().unwrap().to_string_lossy();
        assert!(!name.contains('/'));
        assert!(!name.contains('\\'));
        assert!(!name.contains(':'));
        assert!(!name.contains('\0'));
    }

    #[test]
    fn decode_round_trip_synthetic_png() {
        use image::{ImageFormat, RgbaImage};
        use std::io::Cursor;
        let img = RgbaImage::from_fn(8, 8, |_, _| image::Rgba([10, 20, 30, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        let bytes = buf.into_inner();

        let icon = decode(&bytes, "test://synthetic").expect("decoded");
        assert_eq!(icon.width, 8);
        assert_eq!(icon.height, 8);
        assert_eq!(icon.rgba.len(), 8 * 8 * 4);
    }

    #[test]
    fn decode_reports_failure_on_garbage() {
        let err = decode(b"not an image", "test://garbage").unwrap_err();
        assert!(matches!(err, IconFetchError::Decode { .. }));
    }
}
