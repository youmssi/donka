//! Telling the project's owners that a population moved (DNK-57).
//!
//! The alert is queued in the same transaction as the snapshot that found the crossing, so a
//! crash between the two cannot lose it or send it twice, and delivered by the shared email
//! worker. Owners arrive as a parameter rather than being queried here, so this module reaches
//! into no other module's tables.

use crate::snapshot::Snapshot;
use crate::stability::{Band, Shift};
use crate::{Analysis, AnalysisError};
use chrono::{DateTime, Duration, Utc};
use donka_mail::{Email, Mailer};
use uuid::Uuid;

/// How many emails one delivery pass sends before yielding.
const BATCH: usize = 20;
/// Attempts before an email is abandoned, matching the approval outbox.
const MAX_ATTEMPTS: i32 = 5;

/// The language an alert is written in. Mirrors the release module's own, so a recipient reads
/// the same two languages everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Fr,
}

impl Language {
    /// The segment the web app serves this language under.
    fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}

/// An owner to tell. Supplied by the caller, which knows the project's members.
#[derive(Debug, Clone)]
pub struct Recipient {
    pub email: String,
    pub language: Language,
}

/// The project an alert is about: its name for the reader, its key for the link.
#[derive(Debug, Clone, Copy)]
pub struct AlertProject<'a> {
    pub key: &'a str,
    pub name: &'a str,
}

/// Subject and body of one alert.
struct Content {
    subject: String,
    body: String,
}

/// The band, named for a reader rather than for the database.
fn band_words(band: Band, language: Language) -> &'static str {
    match (band, language) {
        (Band::Shifted, Language::En) => "shifted",
        (Band::Moved, Language::En) => "moved",
        (Band::Stable, Language::En) => "stable",
        (Band::Shifted, Language::Fr) => "a changé",
        (Band::Moved, Language::Fr) => "a bougé",
        (Band::Stable, Language::Fr) => "est stable",
    }
}

/// The measures that reached the worst band, so the owner knows what to look at.
fn breached_fields(snapshot: &Snapshot) -> Vec<&str> {
    snapshot
        .shifts
        .iter()
        .filter(|shift| shift.band == Some(snapshot.worst_band))
        .map(|shift: &Shift| shift.field.as_str())
        .collect()
}

/// Tells an owner a measure crossed into a worse band. It names the decision, the measures and
/// the period, and says plainly what the number does not mean: a shift is not proof of a
/// problem, and the analysis measures volume and mix, never risk.
fn stability_breached(
    language: Language,
    project: &str,
    snapshot: &Snapshot,
    link: &str,
) -> Content {
    let decision = &snapshot.decision_key;
    let band = band_words(snapshot.worst_band, language);
    let fields = breached_fields(snapshot).join(", ");
    let index = snapshot
        .shifts
        .iter()
        .filter_map(|shift| shift.index)
        .fold(0.0_f64, f64::max);
    let recent_from = snapshot.recent_from.format("%Y-%m-%d");
    let recent_until = snapshot.recent_until.format("%Y-%m-%d");
    match language {
        Language::En => Content {
            subject: format!("{project}: the population of {decision} {band}"),
            body: format!(
                "Hello,\n\n\
                 A stability snapshot of {decision} in {project} found that the population \
                 {band} between the reference period and {recent_from} to {recent_until}.\n\n\
                 Measures that moved most: {fields}\n\
                 Highest shift index: {index:.3}\n\n\
                 A shift is not proof of a problem. It says the people being decided about are \
                 not the people the rules were tuned on, which is worth understanding before \
                 the outcomes make it obvious. The snapshot measures volume and mix; it does \
                 not measure risk, and it cannot say whether a decision was right.\n\n\
                 {link}\n\n\
                 You are receiving this because you own the project and this measure crossed \
                 into a worse band than the last one you were told about. You will not be told \
                 again until it worsens further.\n"
            ),
        },
        Language::Fr => Content {
            subject: format!("{project} : la population de {decision} {band}"),
            body: format!(
                "Bonjour,\n\n\
                 Un instantané de stabilité de {decision} dans {project} indique que la \
                 population {band} entre la période de référence et la période du \
                 {recent_from} au {recent_until}.\n\n\
                 Mesures qui ont le plus bougé : {fields}\n\
                 Indice d'écart le plus élevé : {index:.3}\n\n\
                 Un écart ne prouve pas qu'il y a un problème. Il indique que les personnes sur \
                 lesquelles les décisions portent ne sont plus celles sur lesquelles les règles \
                 ont été calibrées, ce qu'il vaut mieux comprendre avant que les résultats ne \
                 le rendent évident. L'instantané mesure le volume et la répartition ; il ne \
                 mesure pas le risque et ne dit pas si une décision était juste.\n\n\
                 {link}\n\n\
                 Vous recevez ce message parce que vous êtes propriétaire du projet et que \
                 cette mesure est passée dans une bande plus défavorable que la dernière \
                 signalée. Vous ne serez pas prévenu à nouveau avant qu'elle ne se dégrade \
                 encore.\n"
            ),
        },
    }
}

impl Analysis {
    /// Queues one alert per owner, in the caller's transaction so the alert and the snapshot
    /// that found the crossing commit together.
    pub(crate) async fn queue_alerts(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        snapshot: &Snapshot,
        project: &AlertProject<'_>,
        public_url: &str,
        owners: &[Recipient],
    ) -> Result<(), AnalysisError> {
        for owner in owners {
            // Each owner reads the alert, and the page it links to, in their own language.
            let link = format!(
                "{}/{}/projects/stability/?p={}&decision={}",
                public_url.trim_end_matches('/'),
                owner.language.as_str(),
                project.key,
                snapshot.decision_key,
            );
            let content = stability_breached(owner.language, project.name, snapshot, &link);
            sqlx::query(
                "INSERT INTO stability_alert_emails (id, snapshot_id, recipient, subject, body, \
                 created_at, next_attempt_at) VALUES ($1,$2,$3,$4,$5,$6,$6)",
            )
            .bind(Uuid::new_v4())
            .bind(snapshot.id)
            .bind(&owner.email)
            .bind(&content.subject)
            .bind(&content.body)
            .bind(snapshot.created_at)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    /// Sends the alerts that are due, each in its own transaction, and returns how many went.
    /// Safe to run from several instances: a row is locked while it is being sent, and one that
    /// a shutdown interrupts is rolled back and sent by the next pass.
    pub async fn deliver_due_alerts(
        &self,
        mailer: &dyn Mailer,
        organisation: Option<&str>,
    ) -> Result<usize, AnalysisError> {
        let mut sent = 0;
        for _ in 0..BATCH {
            let now = Utc::now();
            let mut tx = self.pool.begin().await?;
            let due: Option<(Uuid, String, String, String, i32)> = sqlx::query_as(
                "SELECT id, recipient, subject, body, attempts FROM stability_alert_emails \
                 WHERE sent_at IS NULL AND abandoned_at IS NULL AND next_attempt_at <= $1 \
                 ORDER BY next_attempt_at LIMIT 1 FOR UPDATE SKIP LOCKED",
            )
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
            let Some((id, recipient, subject, body, attempts)) = due else {
                tx.rollback().await?;
                break;
            };
            let attempts = attempts.saturating_add(1);
            let email = Email {
                to: recipient,
                subject,
                text: body,
            }
            .signed(organisation);
            match mailer.send(&email).await {
                Ok(()) => {
                    sqlx::query(
                        "UPDATE stability_alert_emails SET sent_at = $2, attempts = $3, \
                         last_error = NULL WHERE id = $1",
                    )
                    .bind(id)
                    .bind(now)
                    .bind(attempts)
                    .execute(&mut *tx)
                    .await?;
                    sent += 1;
                }
                Err(err) => {
                    // The same backoff the approval outbox uses, abandoning after the last
                    // attempt so a dead address cannot hold the queue.
                    let abandoned = attempts >= MAX_ATTEMPTS;
                    sqlx::query(
                        "UPDATE stability_alert_emails SET attempts = $2, last_error = $3, \
                         next_attempt_at = $4, abandoned_at = $5 WHERE id = $1",
                    )
                    .bind(id)
                    .bind(attempts)
                    .bind(err.to_string())
                    .bind(next_attempt(now, attempts))
                    .bind(abandoned.then_some(now))
                    .execute(&mut *tx)
                    .await?;
                    tracing::warn!(%err, attempts, abandoned, "a stability alert could not be sent");
                }
            }
            tx.commit().await?;
        }
        Ok(sent)
    }
}

/// Doubling backoff: a minute, then two, four, eight.
fn next_attempt(now: DateTime<Utc>, attempts: i32) -> DateTime<Utc> {
    let minutes = 1_i64 << attempts.clamp(0, 6);
    now + Duration::minutes(minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stability::Thresholds;

    fn snapshot_with(worst: Band, shifts: Vec<Shift>) -> Snapshot {
        Snapshot {
            id: Uuid::nil(),
            decision_key: "scorecard".to_owned(),
            scheduled: false,
            reference_from: "2026-01-01T00:00:00Z".parse().unwrap(),
            reference_until: "2026-04-01T00:00:00Z".parse().unwrap(),
            recent_from: "2026-10-01T00:00:00Z".parse().unwrap(),
            recent_until: "2026-11-01T00:00:00Z".parse().unwrap(),
            thresholds: Thresholds::default(),
            minimum_bin: 5,
            records_read: 2_000,
            mix: Vec::new(),
            shifts,
            worst_band: worst,
            created_at: "2026-11-01T09:00:00Z".parse().unwrap(),
        }
    }

    fn shift_at(field: &str, index: f64, band: Band) -> Shift {
        Shift {
            field: field.to_owned(),
            index: Some(index),
            band: Some(band),
            reason: None,
            bins: Vec::new(),
            reference_records: 1_000,
            recent_records: 1_000,
            suppressed_bins: 0,
        }
    }

    #[test]
    fn the_alert_names_only_the_measures_that_reached_the_worst_band() {
        let snapshot = snapshot_with(
            Band::Shifted,
            vec![
                shift_at("score", 0.31, Band::Shifted),
                shift_at("applicant.region", 0.12, Band::Moved),
                shift_at("applicant.age", 0.02, Band::Stable),
            ],
        );
        assert_eq!(breached_fields(&snapshot), vec!["score"]);
    }

    #[test]
    fn both_languages_say_what_the_number_does_not_mean() {
        let snapshot = snapshot_with(Band::Shifted, vec![shift_at("score", 0.42, Band::Shifted)]);
        let en = stability_breached(Language::En, "Retail credit", &snapshot, "https://s/x");
        assert!(en.subject.contains("Retail credit"));
        assert!(en.subject.contains("scorecard"));
        assert!(en.body.contains("0.420"), "the index is quoted: {}", en.body);
        assert!(en.body.contains("not proof of a problem"));
        assert!(
            en.body.contains("does not measure risk") || en.body.contains("not measure risk"),
            "the limitation must be stated: {}",
            en.body
        );
        assert!(en.body.contains("https://s/x"));
        assert!(
            en.body.contains("not be told again"),
            "the once-per-crossing rule is explained: {}",
            en.body
        );

        let fr = stability_breached(Language::Fr, "Crédit", &snapshot, "https://s/x");
        assert!(fr.subject.starts_with("Crédit"));
        assert!(fr.body.contains("0,420") || fr.body.contains("0.420"));
        assert!(fr.body.contains("ne prouve pas"));
        assert!(fr.body.contains("ne mesure pas le risque"));
        assert!(fr.body.contains("https://s/x"));
    }

    #[test]
    fn the_period_a_reader_cares_about_is_the_recent_one() {
        let snapshot = snapshot_with(Band::Moved, vec![shift_at("score", 0.15, Band::Moved)]);
        let en = stability_breached(Language::En, "P", &snapshot, "l");
        assert!(en.body.contains("2026-10-01"), "{}", en.body);
        assert!(en.body.contains("2026-11-01"), "{}", en.body);
    }

    #[test]
    fn the_backoff_doubles_and_stops_growing() {
        let now: DateTime<Utc> = "2026-11-01T09:00:00Z".parse().unwrap();
        assert_eq!((next_attempt(now, 0) - now).num_minutes(), 1);
        assert_eq!((next_attempt(now, 1) - now).num_minutes(), 2);
        assert_eq!((next_attempt(now, 2) - now).num_minutes(), 4);
        assert_eq!((next_attempt(now, 3) - now).num_minutes(), 8);
        assert_eq!(
            (next_attempt(now, 40) - now).num_minutes(),
            64,
            "a wild attempt count cannot overflow the shift"
        );
    }
}
