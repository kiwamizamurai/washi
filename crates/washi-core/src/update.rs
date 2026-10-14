use std::time::Duration;

use serde::{Deserialize, Serialize};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/kiwamizamurai/washi/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/kiwamizamurai/washi/releases/";
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Serialize)]
pub struct Release {
    pub current: String,
    pub latest: String,
    pub url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

fn is_plain_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

pub fn parse_latest(json: &str, current: &str) -> Result<Release, String> {
    let release: GithubRelease =
        serde_json::from_str(json).map_err(|e| format!("Unexpected answer from GitHub: {e}"))?;
    if release.draft || release.prerelease {
        return Err("The latest release on GitHub is not a final release".into());
    }
    let latest = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name).to_string();
    if !is_plain_version(&latest) {
        return Err(format!("Unexpected release tag on GitHub: {}", release.tag_name));
    }
    let url = release
        .html_url
        .filter(|u| u.starts_with(RELEASES_PAGE))
        .unwrap_or_else(|| format!("{RELEASES_PAGE}tag/{}", release.tag_name));
    Ok(Release { current: current.to_string(), latest, url })
}

pub fn fetch_latest() -> Result<Release, String> {
    let current = env!("CARGO_PKG_VERSION");
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into();
    let body = agent
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", format!("Washi/{current}"))
        .call()
        .map_err(|e| format!("Could not reach GitHub: {e}"))?
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_string()
        .map_err(|e| format!("Could not read the answer from GitHub: {e}"))?;
    parse_latest(&body, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_without_the_v_and_keeps_the_release_page() {
        let json = r#"{"tag_name":"v2.1.0","html_url":"https://github.com/kiwamizamurai/washi/releases/tag/v2.1.0","draft":false,"prerelease":false,"assets":[]}"#;
        assert_eq!(
            parse_latest(json, "2.0.0").unwrap(),
            Release {
                current: "2.0.0".into(),
                latest: "2.1.0".into(),
                url: "https://github.com/kiwamizamurai/washi/releases/tag/v2.1.0".into(),
            }
        );
    }

    #[test]
    fn a_page_outside_the_washi_releases_is_never_opened() {
        let json = r#"{"tag_name":"v2.1.0","html_url":"https://example.com/phish"}"#;
        assert_eq!(parse_latest(json, "2.0.0").unwrap().url, "https://github.com/kiwamizamurai/washi/releases/tag/v2.1.0");
        let missing = r#"{"tag_name":"v2.1.0"}"#;
        assert_eq!(parse_latest(missing, "2.0.0").unwrap().url, "https://github.com/kiwamizamurai/washi/releases/tag/v2.1.0");
    }

    #[test]
    fn rejects_drafts_prereleases_odd_tags_and_broken_json() {
        assert!(parse_latest(r#"{"tag_name":"v3.0.0","prerelease":true}"#, "2.0.0").is_err());
        assert!(parse_latest(r#"{"tag_name":"v3.0.0","draft":true}"#, "2.0.0").is_err());
        assert!(parse_latest(r#"{"tag_name":"v3.0.0-beta.1"}"#, "2.0.0").is_err());
        assert!(parse_latest(r#"{"tag_name":"nightly"}"#, "2.0.0").is_err());
        assert!(parse_latest(r#"{"message":"API rate limit exceeded"}"#, "2.0.0").is_err());
        assert!(parse_latest("<html>", "2.0.0").is_err());
    }

    #[test]
    #[ignore]
    fn fetches_the_real_latest_release() {
        let release = fetch_latest().unwrap();
        assert!(is_plain_version(&release.latest), "{release:?}");
        assert!(release.url.starts_with(RELEASES_PAGE), "{release:?}");
    }
}
