//! Dated population stability snapshots (DNK-57).
//!
//! A snapshot measures one decision over two periods and keeps counts, shares and indices. It
//! holds no decrypted value and no record identifier, so it survives the records it measured
//! and needs no purging when they age out.

use crate::alerts::{AlertProject, Recipient};
use crate::stability::{self, Band, BinCounts, MixWeek, Shift, Thresholds};
use crate::{Analysis, AnalysisError};
use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_decision_log::aggregate::{Binning, Characteristic, NumericBins};
use donka_project::{Access, Role};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// What to measure, and over which two periods.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotRequest {
    pub decision_key: String,
    /// The period the recent one is compared with.
    pub reference_from: DateTime<Utc>,
    pub reference_until: DateTime<Utc>,
    pub recent_from: DateTime<Utc>,
    pub recent_until: DateTime<Utc>,
    /// The numeric output SSI is taken on, usually the score, with its bin edges. `None`
    /// measures the inputs alone.
    pub output_field: Option<(String, Vec<f64>)>,
    /// The contract inputs CSI is taken on, by dotted path.
    pub input_fields: Vec<String>,
}

/// A stored snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: Uuid,
    pub decision_key: String,
    pub reference_from: DateTime<Utc>,
    pub reference_until: DateTime<Utc>,
    pub recent_from: DateTime<Utc>,
    pub recent_until: DateTime<Utc>,
    pub thresholds: Thresholds,
    pub minimum_bin: u64,
    pub records_read: u64,
    /// The decision mix of the recent period, by week.
    pub mix: Vec<MixWeek>,
    /// One shift per measured characteristic.
    pub shifts: Vec<Shift>,
    /// The worst band across `shifts`; `Stable` when nothing could be measured.
    pub worst_band: Band,
    pub created_at: DateTime<Utc>,
}

/// The name a band is stored and compared under. Ordered so a worsening is a larger number.
fn band_rank(band: Band) -> i16 {
    match band {
        Band::Stable => 0,
        Band::Moved => 1,
        Band::Shifted => 2,
    }
}

fn band_name(band: Band) -> &'static str {
    match band {
        Band::Stable => "stable",
        Band::Moved => "moved",
        Band::Shifted => "shifted",
    }
}

fn band_from_name(name: &str) -> Band {
    match name {
        "shifted" => Band::Shifted,
        "moved" => Band::Moved,
        _ => Band::Stable,
    }
}

impl Analysis {
    /// Measures one decision and stores the snapshot. Owners only, like every other analysis
    /// read: a snapshot is derived from production decisions about people.
    ///
    /// Returns the snapshot and, when a measure crossed into a worse band than the one last
    /// alerted on, the band to alert for. The caller sends the mail, so this crate keeps no
    /// mailer and the alert cannot be sent twice for the same crossing.
    #[allow(clippy::too_many_arguments)]
    pub async fn snapshot(
        &self,
        access: &Access,
        request: &SnapshotRequest,
        thresholds: Thresholds,
        minimum_bin: u64,
        alerts: Option<&AlertContext<'_>>,
    ) -> Result<(Snapshot, Option<Band>), AnalysisError> {
        access.require(Role::Owner)?;
        if !self.enabled {
            return Err(AnalysisError::Disabled);
        }

        let characteristics = characteristics(request);

        // The two periods are aggregated separately: the shift is between them, and the mix is
        // the recent period's, which is the one a reader is asking about.
        let reference = self
            .log
            .aggregate(
                access,
                &request.decision_key,
                request.reference_from,
                request.reference_until,
                &characteristics,
                self.max_records,
            )
            .await?;
        let recent = self
            .log
            .aggregate(
                access,
                &request.decision_key,
                request.recent_from,
                request.recent_until,
                &characteristics,
                self.max_records,
            )
            .await?;

        let shifts: Vec<Shift> = characteristics
            .iter()
            .map(|characteristic| {
                let empty = BinCounts::new();
                stability::shift(
                    &characteristic.path,
                    reference.fields.get(&characteristic.path).unwrap_or(&empty),
                    recent.fields.get(&characteristic.path).unwrap_or(&empty),
                    minimum_bin,
                    thresholds,
                )
            })
            .collect();
        let worst_band = shifts
            .iter()
            .filter_map(|shift| shift.band)
            .max_by_key(|band| band_rank(*band))
            .unwrap_or(Band::Stable);

        let snapshot = Snapshot {
            id: Uuid::new_v4(),
            decision_key: request.decision_key.clone(),
            reference_from: request.reference_from,
            reference_until: request.reference_until,
            recent_from: request.recent_from,
            recent_until: request.recent_until,
            thresholds,
            minimum_bin,
            records_read: reference.records_read + recent.records_read,
            mix: stability::mix(&recent.weeks),
            shifts,
            worst_band,
            created_at: Utc::now(),
        };

        let alert = self.store(access, &snapshot, alerts).await?;
        Ok((snapshot, alert))
    }

    /// Stores the snapshot and decides whether this crossing is worth an alert, in one
    /// transaction so two passes cannot both claim the same crossing.
    async fn store(
        &self,
        access: &Access,
        snapshot: &Snapshot,
        alerts: Option<&AlertContext<'_>>,
    ) -> Result<Option<Band>, AnalysisError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO stability_snapshots (id, project_id, decision_key, started_by, \
             reference_from, reference_until, recent_from, recent_until, \
             stable_below, shifted_at, minimum_bin, records_read, mix, shifts, worst_band, \
             created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
        )
        .bind(snapshot.id)
        .bind(access.project_id())
        .bind(&snapshot.decision_key)
        .bind(access.user_id())
        .bind(snapshot.reference_from)
        .bind(snapshot.reference_until)
        .bind(snapshot.recent_from)
        .bind(snapshot.recent_until)
        .bind(snapshot.thresholds.stable_below)
        .bind(snapshot.thresholds.shifted_at)
        .bind(i32::try_from(snapshot.minimum_bin).unwrap_or(i32::MAX))
        .bind(i32::try_from(snapshot.records_read).unwrap_or(i32::MAX))
        .bind(
            serde_json::to_value(&snapshot.mix)
                .map_err(|e| AnalysisError::ResultSerialization(e.to_string()))?,
        )
        .bind(
            serde_json::to_value(&snapshot.shifts)
                .map_err(|e| AnalysisError::ResultSerialization(e.to_string()))?,
        )
        .bind(band_name(snapshot.worst_band))
        .bind(snapshot.created_at)
        .execute(&mut *tx)
        .await?;

        donka_audit::record(
            &mut tx,
            Event::new(
                snapshot.created_at,
                Some(access.user_id()),
                Action::StabilitySnapshotTaken,
            )
            .in_project(access.project_id())
            .with_details(json!({
                "snapshotId": snapshot.id,
                "decisionKey": snapshot.decision_key,
                "referenceFrom": snapshot.reference_from,
                "referenceUntil": snapshot.reference_until,
                "recentFrom": snapshot.recent_from,
                "recentUntil": snapshot.recent_until,
                "stableBelow": snapshot.thresholds.stable_below,
                "shiftedAt": snapshot.thresholds.shifted_at,
                "minimumBin": snapshot.minimum_bin,
                "recordsRead": snapshot.records_read,
                "worstBand": band_name(snapshot.worst_band),
            })),
        )
        .await?;

        // A breach is alerted once, when it first worsens past the band already alerted on. The
        // row is locked for the decision, so two passes finishing together cannot both send.
        let alerted: Option<(String,)> = sqlx::query_as(
            "SELECT alerted_band FROM stability_alerts \
             WHERE project_id = $1 AND decision_key = $2 FOR UPDATE",
        )
        .bind(access.project_id())
        .bind(&snapshot.decision_key)
        .fetch_optional(&mut *tx)
        .await?;
        let previous = alerted
            .map(|(name,)| band_from_name(&name))
            .unwrap_or(Band::Stable);

        let worsened = band_rank(snapshot.worst_band) > band_rank(previous);
        let crossed_into_breach = worsened && snapshot.worst_band != Band::Stable;
        if band_rank(snapshot.worst_band) != band_rank(previous) {
            // Recovery is recorded too, so a later breach alerts again rather than being
            // swallowed by a band that was alerted months ago.
            sqlx::query(
                "INSERT INTO stability_alerts (project_id, decision_key, alerted_band, \
                 snapshot_id, alerted_at) VALUES ($1,$2,$3,$4,$5) \
                 ON CONFLICT (project_id, decision_key) DO UPDATE \
                 SET alerted_band = $3, snapshot_id = $4, alerted_at = $5",
            )
            .bind(access.project_id())
            .bind(&snapshot.decision_key)
            .bind(band_name(snapshot.worst_band))
            .bind(snapshot.id)
            .bind(snapshot.created_at)
            .execute(&mut *tx)
            .await?;
        }
        // Queued here, inside the same transaction, so the alert and the snapshot that found
        // the crossing commit together: a crash between the two can neither lose the alert nor
        // send it twice. Nothing is queued when alerts are off.
        if crossed_into_breach {
            if let Some(context) = alerts {
                self.queue_alerts(
                    &mut tx,
                    snapshot,
                    &context.project,
                    context.public_url,
                    context.owners,
                )
                .await?;
            }
        }
        tx.commit().await?;

        Ok(crossed_into_breach.then_some(snapshot.worst_band))
    }

    /// The snapshots of one decision, newest first (owners only).
    pub async fn snapshots(
        &self,
        access: &Access,
        decision_key: &str,
        limit: i64,
    ) -> Result<Vec<Snapshot>, AnalysisError> {
        access.require(Role::Owner)?;
        let rows: Vec<StoredSnapshot> = sqlx::query_as(
            "SELECT id, decision_key, reference_from, reference_until, recent_from, \
             recent_until, stable_below, shifted_at, minimum_bin, records_read, mix, shifts, \
             worst_band, created_at FROM stability_snapshots \
             WHERE project_id = $1 AND decision_key = $2 \
             ORDER BY created_at DESC, id DESC LIMIT $3",
        )
        .bind(access.project_id())
        .bind(decision_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(StoredSnapshot::into_snapshot)
            .collect()
    }
}

/// What an alert needs, when alerts are configured on. Absent turns them off, which is the
/// default: the caller passes `None` and nothing is queued.
#[derive(Debug, Clone)]
pub struct AlertContext<'a> {
    pub project: AlertProject<'a>,
    pub public_url: &'a str,
    pub owners: &'a [Recipient],
}

/// The characteristics a request measures: the chosen output first, then each input.
fn characteristics(request: &SnapshotRequest) -> Vec<Characteristic> {
    let mut characteristics = Vec::new();
    if let Some((path, edges)) = &request.output_field {
        characteristics.push(Characteristic {
            path: path.clone(),
            from_input: false,
            binning: Binning::Numeric(NumericBins {
                edges: edges.clone(),
            }),
        });
    }
    for path in &request.input_fields {
        characteristics.push(Characteristic {
            path: path.clone(),
            from_input: true,
            binning: Binning::Value,
        });
    }
    characteristics
}

#[derive(sqlx::FromRow)]
struct StoredSnapshot {
    id: Uuid,
    decision_key: String,
    reference_from: DateTime<Utc>,
    reference_until: DateTime<Utc>,
    recent_from: DateTime<Utc>,
    recent_until: DateTime<Utc>,
    stable_below: f64,
    shifted_at: f64,
    minimum_bin: i32,
    records_read: i32,
    mix: serde_json::Value,
    shifts: serde_json::Value,
    worst_band: String,
    created_at: DateTime<Utc>,
}

impl StoredSnapshot {
    fn into_snapshot(self) -> Result<Snapshot, AnalysisError> {
        Ok(Snapshot {
            id: self.id,
            decision_key: self.decision_key,
            reference_from: self.reference_from,
            reference_until: self.reference_until,
            recent_from: self.recent_from,
            recent_until: self.recent_until,
            thresholds: Thresholds {
                stable_below: self.stable_below,
                shifted_at: self.shifted_at,
            },
            minimum_bin: u64::try_from(self.minimum_bin).unwrap_or(0),
            records_read: u64::try_from(self.records_read).unwrap_or(0),
            mix: serde_json::from_value(self.mix)
                .map_err(|e| AnalysisError::ResultSerialization(e.to_string()))?,
            shifts: serde_json::from_value(self.shifts)
                .map_err(|e| AnalysisError::ResultSerialization(e.to_string()))?,
            worst_band: band_from_name(&self.worst_band),
            created_at: self.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_name_survives_a_round_trip() {
        for band in [Band::Stable, Band::Moved, Band::Shifted] {
            assert_eq!(band_from_name(band_name(band)), band);
        }
    }

    #[test]
    fn an_unknown_band_name_reads_as_stable_rather_than_a_breach() {
        // The column is constrained, so this only guards against a future value arriving from a
        // newer instance: read it as the quiet end rather than alerting on a name we know nothing about.
        assert_eq!(band_from_name("something-new"), Band::Stable);
    }

    #[test]
    fn worsening_is_ordered_and_recovery_is_not_a_breach() {
        assert!(band_rank(Band::Shifted) > band_rank(Band::Moved));
        assert!(band_rank(Band::Moved) > band_rank(Band::Stable));
    }

    #[test]
    fn the_chosen_output_is_measured_first_then_each_input() {
        let request = SnapshotRequest {
            decision_key: "scorecard".to_owned(),
            reference_from: "2026-01-01T00:00:00Z".parse().unwrap(),
            reference_until: "2026-04-01T00:00:00Z".parse().unwrap(),
            recent_from: "2026-10-01T00:00:00Z".parse().unwrap(),
            recent_until: "2026-11-01T00:00:00Z".parse().unwrap(),
            output_field: Some(("score".to_owned(), vec![500.0, 600.0])),
            input_fields: vec!["applicant.region".to_owned(), "applicant.age".to_owned()],
        };
        let characteristics = characteristics(&request);
        assert_eq!(characteristics.len(), 3);
        assert_eq!(characteristics[0].path, "score");
        assert!(!characteristics[0].from_input, "the score is an output");
        assert!(matches!(characteristics[0].binning, Binning::Numeric(_)));
        assert_eq!(characteristics[1].path, "applicant.region");
        assert!(
            characteristics[1].from_input,
            "a contract field is an input"
        );
        assert_eq!(characteristics[1].binning, Binning::Value);
    }

    #[test]
    fn inputs_alone_can_be_measured_without_a_chosen_output() {
        let request = SnapshotRequest {
            decision_key: "scorecard".to_owned(),
            reference_from: "2026-01-01T00:00:00Z".parse().unwrap(),
            reference_until: "2026-04-01T00:00:00Z".parse().unwrap(),
            recent_from: "2026-10-01T00:00:00Z".parse().unwrap(),
            recent_until: "2026-11-01T00:00:00Z".parse().unwrap(),
            output_field: None,
            input_fields: vec!["applicant.region".to_owned()],
        };
        let characteristics = characteristics(&request);
        assert_eq!(characteristics.len(), 1);
        assert!(characteristics[0].from_input);
    }
}
