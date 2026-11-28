//! Population stability arithmetic: the decision mix over time, and the shift indices risk
//! teams use (DNK-57).
//!
//! Nothing here reads the database or holds a decrypted value. It works on bin counts the
//! decision log produced, so a caller cannot pass a person's data in by accident.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Shift indices below this are read as a stable population.
pub const STABLE_BELOW: f64 = 0.1;
/// Shift indices at or above this are read as a population that moved.
pub const SHIFTED_AT: f64 = 0.25;

/// What a shift index says about one measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Band {
    /// Below the lower threshold.
    Stable,
    /// Between the thresholds: worth watching, not yet a breach.
    Moved,
    /// At or above the upper threshold.
    Shifted,
}

/// The thresholds a snapshot was measured against, recorded with it so a later reader is not
/// left guessing which configuration produced a band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    pub stable_below: f64,
    pub shifted_at: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            stable_below: STABLE_BELOW,
            shifted_at: SHIFTED_AT,
        }
    }
}

impl Thresholds {
    /// The band an index falls in. `shifted_at` wins when the two are equal, so a
    /// misconfiguration cannot hide a breach.
    pub fn band(&self, index: f64) -> Band {
        if index >= self.shifted_at {
            Band::Shifted
        } else if index >= self.stable_below {
            Band::Moved
        } else {
            Band::Stable
        }
    }
}

/// Why a measure has no index. A period that cannot answer says so instead of showing a number
/// that would be read as "stable".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Unmeasurable {
    /// Fewer records than the configured minimum, in one period or both.
    TooFewRecords,
    /// The field held no comparable value in either period.
    NoComparableValues,
}

/// One characteristic's shift between a reference period and a recent one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Shift {
    /// The output field for SSI, or the dotted input path for CSI.
    pub field: String,
    /// `None` when the measure could not be taken; `reason` then says why.
    pub index: Option<f64>,
    pub band: Option<Band>,
    pub reason: Option<Unmeasurable>,
    /// Per-bin detail, suppressed bins already removed.
    pub bins: Vec<ShiftBin>,
    pub reference_records: u64,
    pub recent_records: u64,
    /// Bins dropped because either period held fewer records than the minimum.
    pub suppressed_bins: usize,
}

/// One bin's share in each period, and its contribution to the index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShiftBin {
    pub label: String,
    pub reference_share: f64,
    pub recent_share: f64,
    pub contribution: f64,
}

/// Counts per bin for one period, as the decision log aggregated them.
pub type BinCounts = BTreeMap<String, u64>;

/// Replaces a zero share so the logarithm stays finite. The convention credit teams use: a bin
/// that is empty in one period is treated as holding half a record's worth of the total, which
/// keeps the index large without making it infinite.
fn floor_share(count: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let share = count as f64 / total as f64;
    if share > 0.0 {
        share
    } else {
        0.5 / total as f64
    }
}

/// The shift index of one characteristic between two periods.
///
/// This is the index known as PSI on a score and CSI on an input: the same arithmetic,
/// `sum((recent - reference) * ln(recent / reference))` over the bins, named for what it is
/// applied to. Bins below `min_bin` in either period are dropped before the sum, so a bin can
/// never be small enough to point at one person; `suppressed_bins` records how many went.
pub fn shift(
    field: &str,
    reference: &BinCounts,
    recent: &BinCounts,
    min_bin: u64,
    thresholds: Thresholds,
) -> Shift {
    let reference_records: u64 = reference.values().sum();
    let recent_records: u64 = recent.values().sum();

    let mut labels: Vec<&String> = reference.keys().chain(recent.keys()).collect();
    labels.sort();
    labels.dedup();

    let mut kept: Vec<(&str, u64, u64)> = Vec::new();
    let mut suppressed_bins = 0;
    for label in labels {
        let in_reference = reference.get(label).copied().unwrap_or(0);
        let in_recent = recent.get(label).copied().unwrap_or(0);
        // A bin present in one period and absent from the other still carries the shift, so
        // only a bin that is small *and* non-empty is suppressed: an empty bin names nobody.
        let too_small =
            (in_reference > 0 && in_reference < min_bin) || (in_recent > 0 && in_recent < min_bin);
        if too_small {
            suppressed_bins += 1;
            continue;
        }
        kept.push((label.as_str(), in_reference, in_recent));
    }

    let unmeasurable = if reference_records < min_bin || recent_records < min_bin {
        Some(Unmeasurable::TooFewRecords)
    } else if kept.is_empty() {
        Some(Unmeasurable::NoComparableValues)
    } else {
        None
    };
    if let Some(reason) = unmeasurable {
        return Shift {
            field: field.to_owned(),
            index: None,
            band: None,
            reason: Some(reason),
            bins: Vec::new(),
            reference_records,
            recent_records,
            suppressed_bins,
        };
    }

    // Shares are taken over the kept bins, so suppression cannot make them sum past one.
    let reference_kept: u64 = kept.iter().map(|(_, r, _)| *r).sum();
    let recent_kept: u64 = kept.iter().map(|(_, _, c)| *c).sum();

    let mut bins = Vec::with_capacity(kept.len());
    let mut index = 0.0;
    for (label, in_reference, in_recent) in kept {
        let reference_share = floor_share(in_reference, reference_kept);
        let recent_share = floor_share(in_recent, recent_kept);
        let contribution = (recent_share - reference_share) * (recent_share / reference_share).ln();
        index += contribution;
        bins.push(ShiftBin {
            label: label.to_owned(),
            reference_share,
            recent_share,
            contribution,
        });
    }

    Shift {
        field: field.to_owned(),
        index: Some(index),
        band: Some(thresholds.band(index)),
        reason: None,
        bins,
        reference_records,
        recent_records,
        suppressed_bins,
    }
}

/// One week of the decision mix: how the outcomes divided that week.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MixWeek {
    /// The Monday of the week, as `YYYY-MM-DD`.
    pub week: String,
    pub records: u64,
    /// Share per outcome, summing to one when the week holds any record.
    pub shares: BTreeMap<String, f64>,
}

/// The decision mix per week, from the per-week outcome counts the decision log aggregated.
/// Weeks are returned oldest first so a reader follows the drift forward.
pub fn mix(weeks: &BTreeMap<String, BinCounts>) -> Vec<MixWeek> {
    weeks
        .iter()
        .map(|(week, counts)| {
            let records: u64 = counts.values().sum();
            let shares = counts
                .iter()
                .map(|(outcome, count)| {
                    let share = if records == 0 {
                        0.0
                    } else {
                        *count as f64 / records as f64
                    };
                    (outcome.clone(), share)
                })
                .collect();
            MixWeek {
                week: week.clone(),
                records,
                shares,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "stability/tests.rs"]
mod tests;
