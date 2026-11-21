//! Whether a newer Donka release is published (DNK-64), for administrators to plan an upgrade.
//!
//! Studio asks the release feed (`DONKA_UPDATE_CHECK_URL`, GitHub's Releases API for this
//! repository by default) when it starts and once a day after that (an hour after a failure). With
//! `DONKA_UPDATE_CHECK=false`, the setting of installations without internet access, it never
//! makes the call. A failed check is logged and changes nothing.

use chrono::{DateTime, Utc};
use semver::Version;
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// The latest release of Donka Studio, as GitHub publishes it.
pub const DEFAULT_URL: &str = "https://api.github.com/repos/youmssi/donka/releases/latest";
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);
/// After a failed check (no network yet, the feed down), sooner than a day.
const RETRY_AFTER: Duration = Duration::from_secs(3600);

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub version: String,
    /// Its release notes.
    pub url: String,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct Updates {
    inner: Arc<Inner>,
}

struct Inner {
    current: String,
    /// The feed; `None` when checks are off.
    url: Option<String>,
    http: reqwest::Client,
    latest: RwLock<Option<Published>>,
}

impl Updates {
    /// `current` is this Studio's version; `url` is the feed, `None` to never check.
    pub fn new(current: &str, url: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            // GitHub's API refuses requests without one.
            .user_agent(format!("donka-studio/{current}"))
            .build()
            .unwrap_or_default();
        Self {
            inner: Arc::new(Inner {
                current: current.to_owned(),
                url,
                http,
                latest: RwLock::new(None),
            }),
        }
    }

    pub fn current(&self) -> &str {
        &self.inner.current
    }

    /// Whether Studio checks the feed at all.
    pub fn enabled(&self) -> bool {
        self.inner.url.is_some()
    }

    /// The latest published release, when it is newer than this Studio. Nothing is newer than a
    /// build that is not a release (`edge`, `sha-…`).
    pub fn newer(&self) -> Option<Published> {
        let latest = self
            .inner
            .latest
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()?;
        is_newer(&self.inner.current, &latest.version).then_some(latest)
    }

    /// Reads the feed now.
    pub async fn check(&self) -> Result<(), String> {
        let Some(url) = &self.inner.url else {
            return Ok(());
        };
        let response = self
            .inner
            .http
            .get(url)
            .header("accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|err| format!("{url}: {err}"))?;
        if !response.status().is_success() {
            return Err(format!("{url} answered {}", response.status()));
        }
        let body = response.bytes().await.map_err(|err| err.to_string())?;
        let published = parse(&body)?;
        *self
            .inner
            .latest
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(published);
        Ok(())
    }
}

/// Checks the feed now and once a day, until Studio stops; returns at once when checks are off.
pub async fn watch(updates: Updates) {
    if !updates.enabled() {
        tracing::info!("checking for new Donka releases is off (DONKA_UPDATE_CHECK=false)");
        return;
    }
    loop {
        let next = match updates.check().await {
            Ok(()) => {
                if let Some(newer) = updates.newer() {
                    tracing::info!(version = %newer.version, url = %newer.url, "a newer Donka release is published");
                }
                CHECK_EVERY
            }
            Err(err) => {
                tracing::warn!(%err, "cannot check for new Donka releases; trying again in an hour");
                RETRY_AFTER
            }
        };
        tokio::time::sleep(next).await;
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    published_at: Option<DateTime<Utc>>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

fn parse(body: &[u8]) -> Result<Published, String> {
    let release: Release = serde_json::from_slice(body)
        .map_err(|err| format!("the release feed is not readable: {err}"))?;
    if release.draft || release.prerelease {
        return Err(format!(
            "the feed's latest release {} is not final",
            release.tag_name
        ));
    }
    Ok(Published {
        version: release.tag_name.trim_start_matches('v').to_owned(),
        url: release.html_url,
        published_at: release.published_at,
    })
}

fn is_newer(current: &str, latest: &str) -> bool {
    match (
        Version::parse(current.trim_start_matches('v')),
        Version::parse(latest.trim_start_matches('v')),
    ) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_github_release_is_read() {
        let body = br#"{"tag_name": "v1.6.0", "html_url": "https://github.com/youmssi/donka/releases/tag/v1.6.0",
            "published_at": "2026-10-20T09:00:00Z", "draft": false, "prerelease": false, "assets": []}"#;
        let published = parse(body).unwrap();
        assert_eq!(published.version, "1.6.0");
        assert_eq!(
            published.url,
            "https://github.com/youmssi/donka/releases/tag/v1.6.0"
        );
        assert!(published.published_at.is_some());
        assert!(
            parse(br#"{"tag_name": "v2.0.0-rc.1", "html_url": "x", "prerelease": true}"#).is_err()
        );
        assert!(parse(b"<html>").is_err());
    }

    #[test]
    fn only_a_later_release_is_newer() {
        assert!(is_newer("1.5.0", "1.6.0"));
        assert!(is_newer("v1.5.0", "1.5.1"));
        assert!(!is_newer("1.6.0", "1.6.0"));
        assert!(!is_newer("1.7.0", "1.6.0"));
        assert!(!is_newer("unknown", "1.6.0"));
        assert!(!is_newer("1.5.0", "latest"));
    }

    #[test]
    fn nothing_is_newer_before_a_check_or_when_checks_are_off() {
        let updates = Updates::new("1.5.0", None);
        assert!(!updates.enabled());
        assert_eq!(updates.newer(), None);
    }
}
