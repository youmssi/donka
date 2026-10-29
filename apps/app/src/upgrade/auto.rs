//! Unattended upgrades (DNK-64): `donka-admin auto-upgrade`, run by the installation's
//! `auto-upgrade` service. Only on a staging or demo installation, only inside its weekly window,
//! and only to a release of the same major version: a new major version is announced to
//! administrators and upgraded by hand. Each upgrade is the assisted one (`super::run`):
//! verified, backed up, checked, rolled back on failure, reported.

use super::docker::Engine;
use super::registry::{ImageRef, Registry};
use super::{env_value, read_env, Outcome, DEFAULT_REGISTRY};
use chrono::{Datelike, NaiveDateTime, NaiveTime, Weekday};
use semver::Version;
use std::path::PathBuf;
use std::time::Duration;

/// How often the service looks at the clock and, inside the window, at the registry.
const LOOK_EVERY: Duration = Duration::from_secs(300);

pub struct Settings {
    /// The installation folder.
    pub dir: PathBuf,
    /// The key the new images must be signed with.
    pub key: PathBuf,
    pub credentials: Option<(String, String)>,
}

/// What one look decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Tick {
    /// A production installation (or one that does not say): it never upgrades by itself.
    Production,
    /// Outside the window.
    Closed,
    /// This opening of the window was already used, whatever came of it.
    AlreadyTried,
    /// No later release of the same major version.
    UpToDate,
    /// An upgrade ran; its report is in `upgrades/`.
    Ran(Outcome),
}

/// Looks once at `now`. `last` remembers the window opening already used, so that a refused or
/// rolled-back upgrade is not tried again before the next window.
pub async fn tick(
    settings: &Settings,
    now: NaiveDateTime,
    last: &mut Option<NaiveDateTime>,
) -> Result<Tick, String> {
    let env = read_env(&settings.dir.join(".env"))?;
    let role = env_value(&env, "DONKA_INSTALLATION_ROLE")
        .map(|role| Role::parse(&role))
        .transpose()?
        .unwrap_or_default();
    if !role.upgrades_unattended() {
        return Ok(Tick::Production);
    }
    let window = env_value(&env, "DONKA_AUTO_UPGRADE_WINDOW")
        .ok_or("DONKA_AUTO_UPGRADE_WINDOW is not set in .env, e.g. Sun 02:00-04:00")?;
    let Some(opened) = Window::parse(&window)?.opened_at(now) else {
        return Ok(Tick::Closed);
    };
    if *last == Some(opened) {
        return Ok(Tick::AlreadyTried);
    }
    *last = Some(opened);
    let installed = |key: &str| env_value(&env, key).ok_or_else(|| format!(".env has no {key}"));
    let (studio, runtime) = (
        installed("DONKA_VERSION")?,
        installed("DONKA_RUNTIME_VERSION")?,
    );
    let prefix = env_value(&env, "DONKA_REGISTRY").unwrap_or_else(|| DEFAULT_REGISTRY.to_owned());
    let engine = Engine::connect().await?;
    // As the upgrade itself does: a registry or mirror may only be reachable from there.
    engine.join_network().await?;
    let registry = Registry::new(
        settings.credentials.clone(),
        engine.insecure_registries().await?,
    )?;
    let tags = |name: &str, tag: &str| {
        let image = ImageRef::parse(&format!("{prefix}/{name}:{tag}"));
        let registry = &registry;
        async move { registry.tags(&image?).await }
    };
    let Some(new_studio) = target(&studio, &tags("donka-studio", &studio).await?) else {
        return Ok(Tick::UpToDate);
    };
    // The Runtime moves to its own latest release of the same major version, if there is one.
    let new_runtime = target(&runtime, &tags("donka-runtime", &runtime).await?);
    let report = super::run(super::Options {
        dir: settings.dir.clone(),
        studio: new_studio,
        runtime: new_runtime,
        key: settings.key.clone(),
        credentials: settings.credentials.clone(),
    })
    .await?;
    Ok(Tick::Ran(report.outcome))
}

/// The `auto-upgrade` service: looks every few minutes, in the server's time, until it stops.
/// After an upgrade it has the service recreated with the new release, which ends this one.
pub async fn watch(settings: Settings) {
    let mut last = None;
    loop {
        let now = chrono::Local::now().naive_local();
        match tick(&settings, now, &mut last).await {
            Ok(Tick::Production) => {
                tracing::warn!(
                    "unattended upgrades never run in production: set DONKA_INSTALLATION_ROLE to \
                     staging or demo, or remove the auto-upgrade profile"
                );
                tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
                continue;
            }
            Ok(Tick::Ran(Outcome::Upgraded)) => {
                let restarted = match Engine::connect().await {
                    Ok(engine) => super::restart_upgrader(&engine).await,
                    Err(err) => Err(err),
                };
                if let Err(err) = restarted {
                    tracing::warn!(%err, "upgraded, but the auto-upgrade service still runs the previous release");
                }
            }
            Ok(Tick::Ran(outcome)) => {
                tracing::warn!(
                    ?outcome,
                    "the unattended upgrade did not complete: see upgrades/"
                )
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(%err, "cannot look for an unattended upgrade"),
        }
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

/// What an installation is (`DONKA_INSTALLATION_ROLE`); production unless it says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Production,
    Staging,
    Demo,
}

impl Role {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "production" => Ok(Self::Production),
            "staging" => Ok(Self::Staging),
            "demo" => Ok(Self::Demo),
            other => Err(format!(
                "`{other}` is not a role: production, staging or demo"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::Demo => "demo",
        }
    }

    /// Production never upgrades by itself.
    pub fn upgrades_unattended(self) -> bool {
        !matches!(self, Self::Production)
    }
}

/// A weekly window, `Sun 02:00-04:00`, in the server's time. It may run past midnight
/// (`Sat 23:00-01:00` ends on Sunday at 01:00).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    day: Weekday,
    start: NaiveTime,
    end: NaiveTime,
}

impl Window {
    pub fn parse(text: &str) -> Result<Self, String> {
        let invalid = || format!("`{text}` is not a weekly window such as `Sun 02:00-04:00`");
        let (day, hours) = text.trim().split_once(' ').ok_or_else(invalid)?;
        let day: Weekday = day.parse().map_err(|_| invalid())?;
        let (start, end) = hours.trim().split_once('-').ok_or_else(invalid)?;
        let time = |t: &str| NaiveTime::parse_from_str(t.trim(), "%H:%M").map_err(|_| invalid());
        let (start, end) = (time(start)?, time(end)?);
        if start == end {
            return Err(invalid());
        }
        Ok(Self { day, start, end })
    }

    /// When the occurrence of the window that contains `now` started, if `now` is in one.
    pub fn opened_at(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        let today = now.date();
        let yesterday = today.pred_opt()?;
        [today, yesterday].into_iter().find_map(|day| {
            if day.weekday() != self.day {
                return None;
            }
            let start = day.and_time(self.start);
            let end = if self.end > self.start {
                day.and_time(self.end)
            } else {
                day.succ_opt()?.and_time(self.end)
            };
            (start <= now && now < end).then_some(start)
        })
    }
}

impl std::fmt::Display for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}-{}",
            self.day,
            self.start.format("%H:%M"),
            self.end.format("%H:%M")
        )
    }
}

/// The latest release among `tags` with the same major version as `installed` and later than
/// it; `None` when there is none, or when `installed` is not a release.
pub fn target(installed: &str, tags: &[String]) -> Option<String> {
    let installed = release(installed)?;
    tags.iter()
        .filter_map(|tag| release(tag).map(|version| (version, tag)))
        .filter(|(version, _)| {
            version.major == installed.major && version.pre.is_empty() && *version > installed
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, tag)| tag.clone())
}

fn release(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(date: &str, time: &str) -> NaiveDateTime {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_time(NaiveTime::parse_from_str(time, "%H:%M").unwrap())
    }

    #[test]
    fn production_never_upgrades_by_itself() {
        assert_eq!(Role::default(), Role::Production);
        assert!(!Role::parse("production").unwrap().upgrades_unattended());
        assert!(Role::parse("staging").unwrap().upgrades_unattended());
        assert!(Role::parse("demo").unwrap().upgrades_unattended());
        assert!(Role::parse("prod").is_err());
    }

    #[test]
    fn a_weekly_window_opens_on_its_day_only() {
        // 2026-11-01 is a Sunday.
        let window = Window::parse("Sun 02:00-04:00").unwrap();
        assert_eq!(window.to_string(), "Sun 02:00-04:00");
        assert_eq!(
            window.opened_at(at("2026-11-01", "02:00")),
            Some(at("2026-11-01", "02:00"))
        );
        assert_eq!(
            window.opened_at(at("2026-11-01", "03:59")),
            Some(at("2026-11-01", "02:00"))
        );
        assert_eq!(window.opened_at(at("2026-11-01", "04:00")), None);
        assert_eq!(window.opened_at(at("2026-11-01", "01:59")), None);
        assert_eq!(window.opened_at(at("2026-11-02", "03:00")), None);
    }

    #[test]
    fn a_window_may_run_past_midnight() {
        let window = Window::parse("Sat 23:00-01:00").unwrap();
        assert_eq!(
            window.opened_at(at("2026-10-31", "23:30")),
            Some(at("2026-10-31", "23:00"))
        );
        assert_eq!(
            window.opened_at(at("2026-11-01", "00:30")),
            Some(at("2026-10-31", "23:00"))
        );
        assert_eq!(window.opened_at(at("2026-11-01", "01:00")), None);
    }

    #[test]
    fn invalid_windows_are_refused() {
        for text in [
            "Funday 02:00-04:00",
            "Sun 2-4",
            "Sun 02:00",
            "02:00-04:00",
            "Sun 02:00-02:00",
        ] {
            assert!(Window::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn the_target_is_the_latest_release_of_the_same_major_version() {
        let tags: Vec<String> = [
            "1.4.0",
            "1.5.0",
            "1.5.2",
            "1.6.0-rc.1",
            "2.0.0",
            "latest",
            "sha-abc",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(target("1.4.0", &tags).as_deref(), Some("1.5.2"));
        assert_eq!(target("1.5.2", &tags), None);
        assert_eq!(target("2.0.0", &tags), None);
        assert_eq!(target("edge", &tags), None);
    }
}
