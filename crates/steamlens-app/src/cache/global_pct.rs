use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const API_HOST: &str = "http://api.steampowered.com";
const API_PATH: &str = "/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v0002/";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Debug, thiserror::Error)]
pub enum GlobalPctError {
    #[error("HTTP request failed: {source}")]
    Http {
        #[source]
        source: reqwest::Error,
    },
    #[error("API returned status {status}")]
    Status { status: u16 },
    #[error("JSON parse failed: {source}")]
    Parse {
        #[source]
        source: serde_json::Error,
    },
    #[error("I/O error at {path}: {source}", path = .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(serde::Deserialize)]
struct ApiEnvelope {
    achievementpercentages: ApiInner,
}

#[derive(serde::Deserialize)]
struct ApiInner {
    achievements: Vec<ApiAchievement>,
}

#[derive(serde::Deserialize)]
struct ApiAchievement {
    name: String,
    percent: f32,
}

fn cache_path(app_id: u32) -> PathBuf {
    crate::paths::cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("global_percentages.json")
}

pub async fn load_or_fetch(app_id: u32) -> Result<HashMap<String, f32>, GlobalPctError> {
    let path = cache_path(app_id);
    if let Ok(meta) = tokio::fs::metadata(&path).await
        && let Ok(modified) = meta.modified()
        && SystemTime::now()
            .duration_since(modified)
            .map(|age| age < CACHE_TTL)
            .unwrap_or(false)
        && let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(map) = parse_json(&bytes)
    {
        return Ok(map);
    }

    let bytes = fetch(app_id).await?;
    let map = parse_json(&bytes)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| GlobalPctError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
    }
    let _ = tokio::fs::write(&path, &bytes).await;
    Ok(map)
}

async fn fetch(app_id: u32) -> Result<Vec<u8>, GlobalPctError> {
    let url = format!("{API_HOST}{API_PATH}?gameid={app_id}&format=json");
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|source| GlobalPctError::Http { source })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(GlobalPctError::Status {
            status: status.as_u16(),
        });
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|source| GlobalPctError::Http { source })
}

fn parse_json(bytes: &[u8]) -> Result<HashMap<String, f32>, GlobalPctError> {
    let env: ApiEnvelope =
        serde_json::from_slice(bytes).map_err(|source| GlobalPctError::Parse { source })?;
    Ok(env
        .achievementpercentages
        .achievements
        .into_iter()
        .map(|a| (a.name, a.percent))
        .collect())
}

fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .expect("reqwest::Client init for global_pct failed")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_response() {
        let json = br#"{
            "achievementpercentages": {
                "achievements": [
                    { "name": "ACH_A", "percent": 65.5 },
                    { "name": "ACH_B", "percent": 1.2 }
                ]
            }
        }"#;
        let map = parse_json(json).expect("parsed");
        assert_eq!(map.len(), 2);
        assert!((map["ACH_A"] - 65.5).abs() < 0.001);
        assert!((map["ACH_B"] - 1.2).abs() < 0.001);
    }

    #[test]
    fn parses_empty_array() {
        let json = br#"{"achievementpercentages":{"achievements":[]}}"#;
        let map = parse_json(json).expect("parsed");
        assert!(map.is_empty());
    }

    #[test]
    fn reports_parse_error_on_malformed_input() {
        let err = parse_json(b"not json").unwrap_err();
        assert!(matches!(err, GlobalPctError::Parse { .. }));
    }

    #[test]
    fn reports_parse_error_on_missing_top_level_key() {
        let err = parse_json(br#"{"foo":"bar"}"#).unwrap_err();
        assert!(matches!(err, GlobalPctError::Parse { .. }));
    }
}
