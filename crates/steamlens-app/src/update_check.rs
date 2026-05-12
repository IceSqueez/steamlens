use std::time::Duration;

use semver::Version;
use serde::Deserialize;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/IceSqueez/steamlens/releases/latest";
const USER_AGENT: &str = concat!("steamlens/", env!("CARGO_PKG_VERSION"));
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest: String,
    pub html_url: String,
}

#[derive(Deserialize)]
struct ReleaseDto {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let current_raw = env!("CARGO_PKG_VERSION");
    let current = Version::parse(current_raw).map_err(|e| format!("current semver: {e}"))?;

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http status: {e}"))?;

    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    let dto: ReleaseDto = serde_json::from_str(&body).map_err(|e| format!("parse json: {e}"))?;

    if dto.draft {
        return Ok(None);
    }

    let stripped = dto.tag_name.strip_prefix('v').unwrap_or(&dto.tag_name);
    let latest = Version::parse(stripped).map_err(|e| format!("latest semver: {e}"))?;

    if current.pre.is_empty() && dto.prerelease {
        return Ok(None);
    }

    if latest > current {
        Ok(Some(UpdateInfo {
            latest: stripped.to_owned(),
            html_url: dto.html_url,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_pkg_version() {
        Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION must be valid semver");
    }

    #[test]
    fn semver_orders_alpha_releases() {
        let a = Version::parse("1.0.0-alpha.8").unwrap();
        let b = Version::parse("1.0.0-alpha.9").unwrap();
        assert!(b > a);
    }

    #[test]
    fn semver_ignores_build_metadata() {
        let a = Version::parse("1.0.0-alpha.8").unwrap();
        let b = Version::parse("1.0.0-alpha.8+build.1").unwrap();
        assert_eq!(a.cmp_precedence(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn stable_user_skips_prerelease() {
        let dto = ReleaseDto {
            tag_name: "v1.0.0-rc.1".to_owned(),
            html_url: "https://example.com/r".to_owned(),
            draft: false,
            prerelease: true,
        };
        let current = Version::parse("1.0.0").unwrap();
        let stripped = dto.tag_name.strip_prefix('v').unwrap();
        let latest = Version::parse(stripped).unwrap();
        let skip = current.pre.is_empty() && dto.prerelease;
        assert!(skip);
        assert!(latest < current);
    }
}
