//! Binned aggregates over recorded decisions, for population stability (DNK-57).
//!
//! Every function here opens records to read one field and returns **counts per bin only**. No
//! decrypted value, and no record identifier, leaves this module: a caller cannot use it to read
//! a person's data, which is what lets stability snapshots be kept and shown without holding
//! anything personal.
//!
//! One audit event covers a whole aggregate, naming the period and the bins. Reading ten
//! thousand records through [`DecisionLog::view`] would write ten thousand audit rows and bury
//! the log a scheduled snapshot is meant to leave readable.

use crate::{DecisionLog, DecisionLogError, Filter, Status};
use chrono::{DateTime, Datelike, Utc};
use donka_audit::{Action, Event};
use donka_project::Access;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Counts per bin label.
pub type BinCounts = BTreeMap<String, u64>;

/// How many records one aggregate pass opens at a time.
const PAGE: i64 = 200;

/// What to bin a numeric field into. A value's bin is the first range it falls below, so the
/// edges read as "up to 500", "up to 600" and so on, with everything above the last edge in one
/// final bin.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericBins {
    pub edges: Vec<f64>,
}

impl NumericBins {
    /// The label for `value`. Edges are taken in order, so they must be sorted; an unsorted
    /// configuration still produces stable labels, just not useful ones.
    fn label(&self, value: f64) -> String {
        for edge in &self.edges {
            if value < *edge {
                return format!("<{edge}");
            }
        }
        match self.edges.last() {
            Some(last) => format!(">={last}"),
            None => "all".to_owned(),
        }
    }
}

/// How one field is binned.
#[derive(Debug, Clone, PartialEq)]
pub enum Binning {
    /// Numbers fall into configured ranges.
    Numeric(NumericBins),
    /// Anything else is binned by its own value: a string, a boolean, or a number read as a
    /// label. A value that is absent or null bins as `missing`, so a field that stopped being
    /// sent shows as a shift rather than vanishing from the comparison.
    Value,
}

/// A field to aggregate: where to read it from, and how to bin it.
#[derive(Debug, Clone, PartialEq)]
pub struct Characteristic {
    /// A dotted path, `applicant.age` or `score`.
    pub path: String,
    /// Read from the record's input rather than its output.
    pub from_input: bool,
    pub binning: Binning,
}

/// The value at a dotted path, descending through objects only.
fn at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

/// The bin a record's value falls in, or `missing` when the field is absent or null.
fn bin_of(payload: Option<&Value>, characteristic: &Characteristic) -> String {
    let Some(value) = payload.and_then(|root| at_path(root, &characteristic.path)) else {
        return "missing".to_owned();
    };
    if value.is_null() {
        return "missing".to_owned();
    }
    match &characteristic.binning {
        Binning::Numeric(bins) => match value.as_f64() {
            Some(number) => bins.label(number),
            // A field configured as numeric that arrives as something else is not silently
            // counted as a number; it bins apart so the reader sees it happened.
            None => "notANumber".to_owned(),
        },
        Binning::Value => match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        },
    }
}

/// The Monday of the week `at` falls in, as `YYYY-MM-DD`. Weeks are ISO weeks, so a mix read in
/// one installation lines up with a report written in another.
fn week_of(at: DateTime<Utc>) -> String {
    let date = at.date_naive();
    let monday = date - chrono::Duration::days(i64::from(date.weekday().num_days_from_monday()));
    monday.format("%Y-%m-%d").to_string()
}

/// One record, reduced to what an aggregate needs.
struct Reduced {
    evaluated_at: DateTime<Utc>,
    outcome: Option<String>,
    bins: Vec<String>,
}

impl DecisionLog {
    /// Counts production outcomes per ISO week over `from..until`, and the bin counts of each
    /// characteristic, for one decision.
    ///
    /// Only successful records are aggregated: a failed decision has no outcome to divide and no
    /// score to compare, so counting it would move an index for a reason that is not a
    /// population shift.
    ///
    /// Returns the weekly outcome counts and, per characteristic path, its bin counts. Audited
    /// once, with the period, the fields and how many records were read.
    pub async fn aggregate(
        &self,
        access: &Access,
        decision_key: &str,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
        characteristics: &[Characteristic],
        max_records: usize,
    ) -> Result<Aggregated, DecisionLogError> {
        let filter = Filter {
            decision_key: Some(decision_key.to_owned()),
            environment: Some(crate::Environment::Production),
            status: Some(Status::Succeeded),
            from: Some(from),
            to: Some(until),
            ..Filter::default()
        };

        let mut weeks: BTreeMap<String, BinCounts> = BTreeMap::new();
        let mut per_field: BTreeMap<String, BinCounts> = characteristics
            .iter()
            .map(|c| (c.path.clone(), BinCounts::new()))
            .collect();
        let mut records_read = 0_usize;
        let mut offset = 0_i64;
        let mut available = 0_u64;

        loop {
            if records_read >= max_records {
                break;
            }
            let page = self
                .search(
                    access,
                    &filter,
                    crate::PageRequest {
                        limit: PAGE,
                        offset,
                    },
                )
                .await?;
            available = u64::try_from(page.total).unwrap_or(u64::MAX);
            if page.items.is_empty() {
                break;
            }
            offset += page.items.len() as i64;
            let ids: Vec<(Uuid, DateTime<Utc>, Option<String>)> = page
                .items
                .into_iter()
                .map(|summary| (summary.id, summary.evaluated_at, summary.outcome))
                .collect();
            for (id, evaluated_at, outcome) in ids {
                if records_read >= max_records {
                    break;
                }
                let reduced = self
                    .reduce(access, id, evaluated_at, outcome, characteristics)
                    .await?;
                records_read += 1;
                let week = weeks.entry(week_of(reduced.evaluated_at)).or_default();
                let outcome = reduced.outcome.unwrap_or_else(|| "missing".to_owned());
                *week.entry(outcome).or_insert(0) += 1;
                for (characteristic, bin) in characteristics.iter().zip(reduced.bins) {
                    let field = per_field
                        .get_mut(&characteristic.path)
                        .expect("every characteristic has an entry");
                    *field.entry(bin).or_insert(0) += 1;
                }
            }
        }

        let mut tx = self.pool.begin().await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                self.clock.now(),
                Some(access.user_id()),
                Action::DecisionRecordsAggregated,
            )
            .in_project(access.project_id())
            .with_details(json!({
                "decisionKey": decision_key,
                "from": from,
                "until": until,
                "recordsRead": records_read,
                "recordsAvailable": available,
                "fields": characteristics.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
            })),
        )
        .await?;
        tx.commit().await?;

        Ok(Aggregated {
            weeks,
            fields: per_field,
            records_read: records_read as u64,
            records_available: available,
        })
    }

    /// Opens one record and keeps only its week, outcome and bins. The decrypted payload is
    /// dropped before this returns, so nothing personal outlives the call.
    ///
    /// This reads the sealed payload without auditing, which [`DecisionLog::view`] never does.
    /// It is private to this module and returns bin labels only: the audit event covering the
    /// whole aggregate names the period, the fields and the number of records, which is what a
    /// reader of the log needs, and what the story asks for. Do not widen its visibility, and do
    /// not return the payload from it.
    async fn reduce(
        &self,
        access: &Access,
        id: Uuid,
        evaluated_at: DateTime<Utc>,
        outcome: Option<String>,
        characteristics: &[Characteristic],
    ) -> Result<Reduced, DecisionLogError> {
        let sealed: Option<(String, Vec<u8>, Vec<u8>)> = sqlx::query_as(
            "SELECT key_id, nonce, payload FROM decision_records \
             WHERE project_id = $1 AND id = $2",
        )
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let (key_id, nonce, ciphertext) = sealed.ok_or(DecisionLogError::RecordNotFound)?;
        let payload = self.unseal(
            id,
            donka_shared::cipher::Sealed {
                key_id,
                nonce,
                ciphertext,
            },
        )?;
        let bins = characteristics
            .iter()
            .map(|characteristic| {
                let root = if characteristic.from_input {
                    Some(&payload.input)
                } else {
                    payload.output.as_ref()
                };
                bin_of(root, characteristic)
            })
            .collect();
        Ok(Reduced {
            evaluated_at,
            outcome,
            bins,
        })
    }
}

/// What one aggregate pass counted. Counts only: no value and no identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregated {
    /// Outcome counts per ISO week, oldest key first.
    pub weeks: BTreeMap<String, BinCounts>,
    /// Bin counts per characteristic path.
    pub fields: BTreeMap<String, BinCounts>,
    pub records_read: u64,
    pub records_available: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(edges: &[f64]) -> Characteristic {
        Characteristic {
            path: "score".to_owned(),
            from_input: false,
            binning: Binning::Numeric(NumericBins {
                edges: edges.to_vec(),
            }),
        }
    }

    fn by_value(path: &str) -> Characteristic {
        Characteristic {
            path: path.to_owned(),
            from_input: true,
            binning: Binning::Value,
        }
    }

    #[test]
    fn a_number_falls_in_the_first_range_it_is_below() {
        let bins = NumericBins {
            edges: vec![500.0, 600.0, 700.0],
        };
        assert_eq!(bins.label(400.0), "<500");
        assert_eq!(bins.label(499.9), "<500");
        assert_eq!(bins.label(500.0), "<600", "an edge belongs to the range above it");
        assert_eq!(bins.label(650.0), "<700");
        assert_eq!(bins.label(700.0), ">=700");
        assert_eq!(bins.label(9000.0), ">=700");
    }

    #[test]
    fn a_numeric_field_with_no_edges_bins_as_one_population() {
        let bins = NumericBins { edges: Vec::new() };
        assert_eq!(bins.label(1.0), "all");
    }

    #[test]
    fn a_dotted_path_descends_through_objects() {
        let payload = json!({"applicant": {"age": 34, "region": "north"}, "score": 612});
        assert_eq!(at_path(&payload, "score"), Some(&json!(612)));
        assert_eq!(at_path(&payload, "applicant.region"), Some(&json!("north")));
        assert_eq!(at_path(&payload, "applicant.missing"), None);
        assert_eq!(at_path(&payload, "score.deeper"), None, "a number has no fields");
    }

    #[test]
    fn an_absent_or_null_field_bins_as_missing() {
        let characteristic = by_value("applicant.region");
        assert_eq!(bin_of(None, &characteristic), "missing");
        assert_eq!(bin_of(Some(&json!({})), &characteristic), "missing");
        assert_eq!(
            bin_of(Some(&json!({"applicant": {"region": null}})), &characteristic),
            "missing",
            "a field that stopped being sent shows as a shift, not as an absence"
        );
    }

    #[test]
    fn a_value_bin_keeps_strings_as_they_are_and_renders_the_rest() {
        let characteristic = by_value("applicant.region");
        assert_eq!(
            bin_of(Some(&json!({"applicant": {"region": "north"}})), &characteristic),
            "north",
            "a string is its own label, not a quoted one"
        );
        let flag = by_value("applicant.resident");
        assert_eq!(
            bin_of(Some(&json!({"applicant": {"resident": true}})), &flag),
            "true"
        );
    }

    #[test]
    fn a_numeric_field_that_arrives_as_something_else_bins_apart() {
        let characteristic = numeric(&[500.0, 600.0]);
        assert_eq!(bin_of(Some(&json!({"score": 550})), &characteristic), "<600");
        assert_eq!(
            bin_of(Some(&json!({"score": "unscored"})), &characteristic),
            "notANumber",
            "a non-number is not silently counted as a number"
        );
    }

    #[test]
    fn a_week_is_named_by_its_monday() {
        // 2026-11-27 is a Friday; its ISO week began Monday 2026-11-23.
        let friday: DateTime<Utc> = "2026-11-27T13:00:00Z".parse().unwrap();
        assert_eq!(week_of(friday), "2026-11-23");
        let monday: DateTime<Utc> = "2026-11-23T00:00:00Z".parse().unwrap();
        assert_eq!(week_of(monday), "2026-11-23", "a Monday is its own week");
        let sunday: DateTime<Utc> = "2026-11-29T23:59:59Z".parse().unwrap();
        assert_eq!(week_of(sunday), "2026-11-23", "a Sunday closes the same week");
        let next: DateTime<Utc> = "2026-11-30T00:00:00Z".parse().unwrap();
        assert_eq!(week_of(next), "2026-11-30", "Monday starts the next one");
    }
}
