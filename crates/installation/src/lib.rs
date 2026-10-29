//! The installation itself (DNK-74): the organisation it serves, and whether its first-run
//! assistant is done.
//!
//! A new installation is in its first run until an administrator finishes the assistant in the
//! browser (second step, organisation, packs). Who may do that is the caller's to check; this
//! module keeps the state and audits each change.

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_shared::clock::Clock;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// The longest organisation name, in characters.
pub const MAX_ORGANISATION_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    #[error(
        "the organisation's name is 1 to {MAX_ORGANISATION_CHARS} characters, without line breaks"
    )]
    InvalidOrganisation,
    /// The first-run assistant was already finished.
    #[error("the installation is already set up")]
    AlreadySetUp,
    /// The first-run assistant names the organisation first.
    #[error("the installation is not set up yet")]
    NotSetUp,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Where the installation stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub organisation: Option<String>,
    /// When the first-run assistant was finished; `None` during the first run.
    pub set_up_at: Option<DateTime<Utc>>,
}

impl Settings {
    /// The first-run assistant is still to be finished.
    pub fn first_run(&self) -> bool {
        self.set_up_at.is_none()
    }
}

#[derive(Clone)]
pub struct Installation {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Installation {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    pub async fn settings(&self) -> Result<Settings, InstallationError> {
        let row: Option<(Option<String>, DateTime<Utc>)> =
            sqlx::query_as("SELECT organisation, set_up_at FROM installation")
                .fetch_optional(&self.pool)
                .await?;
        Ok(match row {
            Some((organisation, set_up_at)) => Settings {
                organisation,
                set_up_at: Some(set_up_at),
            },
            None => Settings {
                organisation: None,
                set_up_at: None,
            },
        })
    }

    /// The organisation's name, for emails and exports; `None` until one is given.
    pub async fn organisation(&self) -> Result<Option<String>, InstallationError> {
        Ok(self.settings().await?.organisation)
    }

    /// Finishes the first run with the organisation's name. Only once.
    pub async fn finish_first_run(
        &self,
        actor: Uuid,
        organisation: &str,
    ) -> Result<Settings, InstallationError> {
        let organisation = valid(organisation)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO installation (organisation, set_up_by, set_up_at) VALUES ($1, $2, $3) \
             ON CONFLICT (only_row) DO NOTHING",
        )
        .bind(&organisation)
        .bind(actor)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            return Err(InstallationError::AlreadySetUp);
        }
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(actor), Action::InstallationSetUp)
                .with_details(json!({ "organisation": organisation })),
        )
        .await?;
        tx.commit().await?;
        Ok(Settings {
            organisation: Some(organisation),
            set_up_at: Some(now),
        })
    }

    /// Changes the organisation's name once the installation is set up.
    pub async fn rename(
        &self,
        actor: Uuid,
        organisation: &str,
    ) -> Result<Settings, InstallationError> {
        let organisation = valid(organisation)?;
        let mut tx = self.pool.begin().await?;
        let row: Option<(Option<String>, DateTime<Utc>)> =
            sqlx::query_as("SELECT organisation, set_up_at FROM installation FOR UPDATE")
                .fetch_optional(&mut *tx)
                .await?;
        let Some((previous, set_up_at)) = row else {
            return Err(InstallationError::NotSetUp);
        };
        if previous.as_deref() != Some(organisation.as_str()) {
            sqlx::query("UPDATE installation SET organisation = $1")
                .bind(&organisation)
                .execute(&mut *tx)
                .await?;
            donka_audit::record(
                &mut tx,
                Event::new(
                    self.clock.now(),
                    Some(actor),
                    Action::InstallationOrganisationRenamed,
                )
                .with_details(json!({ "from": previous, "to": organisation })),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(Settings {
            organisation: Some(organisation),
            set_up_at: Some(set_up_at),
        })
    }
}

/// The name, trimmed; one line of at most [`MAX_ORGANISATION_CHARS`] characters.
fn valid(organisation: &str) -> Result<String, InstallationError> {
    let organisation = organisation.trim();
    let length = organisation.chars().count();
    if length == 0 || length > MAX_ORGANISATION_CHARS || organisation.chars().any(char::is_control)
    {
        return Err(InstallationError::InvalidOrganisation);
    }
    Ok(organisation.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(
            valid("  Microfinance SA ").ok().as_deref(),
            Some("Microfinance SA")
        );
        assert_eq!(
            valid(&"é".repeat(100)).ok().map(|n| n.chars().count()),
            Some(100)
        );
        for bad in ["", "   ", "Two\nlines", "Tab\there", &"x".repeat(101)] {
            assert!(valid(bad).is_err(), "{bad:?}");
        }
    }
}
