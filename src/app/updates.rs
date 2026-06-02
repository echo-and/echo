use std::time::SystemTime;

use anyhow::{Context as _, Result, bail};
use reqwest::{
    StatusCode,
    header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT},
};
use semver::Version;
use serde::Deserialize;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPOSITORY_URL: &str = "https://github.com/echo-and/echo";
pub const GITHUB_RELEASES_URL: &str = "https://github.com/echo-and/echo/releases";
pub const GITHUB_LICENSE_URL: &str = "https://github.com/echo-and/echo/blob/main/LICENSE";

const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/echo-and/echo/releases/latest";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum UpdateStatus {
    #[default]
    NotChecked,
    Checking,
    UpToDate {
        checked_at: SystemTime,
    },
    Available {
        latest_version: String,
        release_url: String,
        checked_at: SystemTime,
    },
    Unavailable {
        reason: UpdateUnavailableReason,
        checked_at: SystemTime,
    },
}

impl UpdateStatus {
    pub fn is_checking(&self) -> bool {
        matches!(self, Self::Checking)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateUnavailableReason {
    NoRelease,
    InvalidRelease,
    RequestFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AvailableUpdate {
    latest_version: String,
    release_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub async fn check_for_updates() -> UpdateStatus {
    let checked_at = SystemTime::now();

    match fetch_latest_release().await {
        Ok(Some(release)) => match available_update_for(CURRENT_VERSION, &release) {
            Ok(Some(update)) => UpdateStatus::Available {
                latest_version: update.latest_version,
                release_url: update.release_url,
                checked_at,
            },
            Ok(None) => UpdateStatus::UpToDate { checked_at },
            Err(()) => UpdateStatus::Unavailable {
                reason: UpdateUnavailableReason::InvalidRelease,
                checked_at,
            },
        },
        Ok(None) => UpdateStatus::Unavailable {
            reason: UpdateUnavailableReason::NoRelease,
            checked_at,
        },
        Err(_) => UpdateStatus::Unavailable {
            reason: UpdateUnavailableReason::RequestFailed,
            checked_at,
        },
    }
}

async fn fetch_latest_release() -> Result<Option<GitHubRelease>> {
    let response = reqwest::Client::new()
        .get(GITHUB_LATEST_RELEASE_API)
        .headers(github_headers()?)
        .send()
        .await
        .context("failed to request latest Echo release")?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        bail!(
            "GitHub latest release request failed with {}",
            response.status()
        );
    }

    let release = response
        .json::<GitHubRelease>()
        .await
        .context("failed to decode latest Echo release")?;

    Ok(Some(release))
}

fn github_headers() -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("echo/{CURRENT_VERSION}"))
            .context("failed to build GitHub user agent")?,
    );
    Ok(headers)
}

fn available_update_for(
    current_version: &str,
    release: &GitHubRelease,
) -> Result<Option<AvailableUpdate>, ()> {
    if release.draft || release.prerelease {
        return Ok(None);
    }

    let current = version_from_tag(current_version).ok_or(())?;
    let latest = version_from_tag(&release.tag_name).ok_or(())?;

    Ok((latest > current).then(|| AvailableUpdate {
        latest_version: release.tag_name.clone(),
        release_url: release.html_url.clone(),
    }))
}

fn version_from_tag(tag: &str) -> Option<Version> {
    let version = tag.trim().trim_start_matches('v');
    Version::parse(version).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag_name: &str) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag_name.to_string(),
            html_url: format!("{GITHUB_RELEASES_URL}/tag/{tag_name}"),
            draft: false,
            prerelease: false,
        }
    }

    #[test]
    fn parses_versions_with_optional_v_prefix() {
        assert_eq!(
            version_from_tag("v1.2.3"),
            Some(Version::parse("1.2.3").unwrap())
        );
        assert_eq!(
            version_from_tag("1.2.3"),
            Some(Version::parse("1.2.3").unwrap())
        );
        assert_eq!(version_from_tag("not-a-version"), None);
    }

    #[test]
    fn detects_newer_release() {
        let update = available_update_for("0.1.0", &release("v0.2.0"))
            .unwrap()
            .unwrap();
        assert_eq!(update.latest_version, "v0.2.0");
    }

    #[test]
    fn ignores_same_or_older_release() {
        assert!(
            available_update_for("0.1.0", &release("v0.1.0"))
                .unwrap()
                .is_none()
        );
        assert!(
            available_update_for("0.2.0", &release("v0.1.0"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn ignores_draft_and_prerelease() {
        let mut draft = release("v0.2.0");
        draft.draft = true;
        assert!(available_update_for("0.1.0", &draft).unwrap().is_none());

        let mut prerelease = release("v0.2.0");
        prerelease.prerelease = true;
        assert!(
            available_update_for("0.1.0", &prerelease)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_invalid_release_versions() {
        assert!(available_update_for("0.1.0", &release("latest")).is_err());
    }
}
