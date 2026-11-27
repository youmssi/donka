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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unmeasurable {
    /// Fewer records than the configured minimum, in one period or both.
    TooFewRecords,
    /// The field held no comparable value in either period.
    NoComparableValues,
}

/// One characteristic's shift between a reference period and a recent one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        let too_small = (in_reference > 0 && in_reference < min_bin)
            || (in_recent > 0 && in_recent < min_bin);
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, u64)]) -> BinCounts {
        pairs
            .iter()
            .map(|(label, count)| ((*label).to_owned(), *count))
            .collect()
    }

    /// The worked example the story asks for, taken through the arithmetic by hand.
    ///
    /// Reference 100 records: A 50, B 30, C 20 -> shares .50 .30 .20
    /// Recent    100 records: A 40, B 30, C 30 -> shares .40 .30 .30
    ///
    /// A: (.40 - .50) * ln(.40 / .50) = -.10 * ln(0.8)      =  0.0223143551
    /// B: (.30 - .30) * ln(.30 / .30) =    0 * 0            =  0
    /// C: (.30 - .20) * ln(.30 / .20) =  .10 * ln(1.5)      =  0.0405465108
    /// index                                                 =  0.0628608659
    #[test]
    fn the_shift_index_matches_the_worked_example() {
        let reference = counts(&[("A", 50), ("B", 30), ("C", 20)]);
        let recent = counts(&[("A", 40), ("B", 30), ("C", 30)]);

        let shift = shift("score", &reference, &recent, 5, Thresholds::default());

        let index = shift.index.expect("the periods hold enough records");
        assert!(
            (index - 0.0628608659).abs() < 1e-9,
            "index was {index}, expected 0.0628608659"
        );
        assert_eq!(shift.band, Some(Band::Stable), "0.063 is below 0.1");
        assert_eq!(shift.suppressed_bins, 0);
        assert_eq!(shift.reference_records, 100);
        assert_eq!(shift.recent_records, 100);

        let b = shift
            .bins
            .iter()
            .find(|bin| bin.label == "B")
            .expect("B is kept");
        assert_eq!(b.contribution, 0.0, "a bin that did not move contributes nothing");
        let c = shift
            .bins
            .iter()
            .find(|bin| bin.label == "C")
            .expect("C is kept");
        assert!((c.contribution - 0.0405465108).abs() < 1e-9);
    }

    #[test]
    fn the_index_is_symmetric_in_the_two_periods() {
        let one = counts(&[("A", 50), ("B", 30), ("C", 20)]);
        let two = counts(&[("A", 40), ("B", 30), ("C", 30)]);
        let forward = shift("score", &one, &two, 5, Thresholds::default())
            .index
            .unwrap();
        let backward = shift("score", &two, &one, 5, Thresholds::default())
            .index
            .unwrap();
        assert!(
            (forward - backward).abs() < 1e-12,
            "{forward} and {backward} should match"
        );
    }

    #[test]
    fn an_unmoved_population_has_a_zero_index() {
        let same = counts(&[("A", 60), ("B", 40)]);
        let shift = shift("score", &same, &same, 5, Thresholds::default());
        assert_eq!(shift.index, Some(0.0));
        assert_eq!(shift.band, Some(Band::Stable));
    }

    #[test]
    fn the_bands_follow_the_configured_thresholds() {
        let thresholds = Thresholds::default();
        assert_eq!(thresholds.band(0.0), Band::Stable);
        assert_eq!(thresholds.band(0.0999), Band::Stable);
        assert_eq!(thresholds.band(0.1), Band::Moved, "the lower bound is inclusive");
        assert_eq!(thresholds.band(0.2499), Band::Moved);
        assert_eq!(thresholds.band(0.25), Band::Shifted, "the upper bound is inclusive");
        assert_eq!(thresholds.band(9.0), Band::Shifted);

        let strict = Thresholds {
            stable_below: 0.05,
            shifted_at: 0.15,
        };
        assert_eq!(strict.band(0.08), Band::Moved, "configuration is honoured");
        assert_eq!(strict.band(0.2), Band::Shifted);
    }

    #[test]
    fn equal_thresholds_report_a_breach_rather_than_hiding_one() {
        let thresholds = Thresholds {
            stable_below: 0.2,
            shifted_at: 0.2,
        };
        assert_eq!(thresholds.band(0.2), Band::Shifted);
    }

    #[test]
    fn a_period_with_too_few_records_says_so_instead_of_showing_a_value() {
        let reference = counts(&[("A", 30), ("B", 20)]);
        let thin = counts(&[("A", 2)]);
        let shift = shift("score", &reference, &thin, 10, Thresholds::default());
        assert_eq!(shift.index, None, "no number is shown");
        assert_eq!(shift.band, None);
        assert_eq!(shift.reason, Some(Unmeasurable::TooFewRecords));
        assert!(shift.bins.is_empty(), "no bin detail escapes either");
        assert_eq!(shift.recent_records, 2);
    }

    #[test]
    fn a_small_non_empty_bin_is_suppressed_and_counted() {
        // "rare" holds 3 records in the recent period, below the minimum of 10.
        let reference = counts(&[("common", 500), ("rare", 400)]);
        let recent = counts(&[("common", 600), ("rare", 3)]);
        let shift = shift("applicant.region", &reference, &recent, 10, Thresholds::default());
        assert_eq!(shift.suppressed_bins, 1);
        assert_eq!(shift.bins.len(), 1, "only the common bin is reported");
        assert_eq!(shift.bins[0].label, "common");
        assert!(
            shift.bins.iter().all(|bin| bin.label != "rare"),
            "a bin that could point at three people is not reported"
        );
        // Shares are taken over the kept bins, so the one remaining bin is the whole population.
        assert_eq!(shift.bins[0].reference_share, 1.0);
        assert_eq!(shift.bins[0].recent_share, 1.0);
        // The totals still describe the real periods, so the reader sees what was read.
        assert_eq!(shift.reference_records, 900);
        assert_eq!(shift.recent_records, 603);
    }

    #[test]
    fn an_empty_bin_still_carries_the_shift() {
        // "declined" vanished entirely. That is the shift, not a small bin to hide.
        let reference = counts(&[("approved", 500), ("declined", 500)]);
        let recent = counts(&[("approved", 1000)]);
        let shift = shift("decision", &reference, &recent, 10, Thresholds::default());
        assert_eq!(shift.suppressed_bins, 0, "an empty bin names nobody");
        let index = shift.index.expect("both periods are large enough");
        assert!(index > SHIFTED_AT, "a vanished outcome is a shift, got {index}");
        assert_eq!(shift.band, Some(Band::Shifted));
        assert!(index.is_finite(), "the floor keeps the logarithm finite");
    }

    #[test]
    fn a_field_with_no_comparable_value_says_so() {
        let empty = BinCounts::new();
        let nothing = shift("applicant.age", &empty, &empty, 1, Thresholds::default());
        assert_eq!(nothing.reason, Some(Unmeasurable::TooFewRecords));

        // Both periods are large enough, but every bin in them is too small to report.
        let reference = counts(&[("a", 1), ("b", 1), ("c", 1), ("d", 1), ("e", 1)]);
        let recent = counts(&[("a", 1), ("b", 1), ("c", 1), ("d", 1), ("e", 1)]);
        let all_suppressed = shift("applicant.age", &reference, &recent, 3, Thresholds::default());
        assert_eq!(all_suppressed.reason, Some(Unmeasurable::NoComparableValues));
        assert_eq!(all_suppressed.suppressed_bins, 5);
        assert_eq!(all_suppressed.index, None);
    }

    #[test]
    fn the_mix_divides_each_week_into_shares() {
        let mut weeks = BTreeMap::new();
        weeks.insert(
            "2026-11-09".to_owned(),
            counts(&[("approved", 30), ("declined", 10)]),
        );
        weeks.insert(
            "2026-11-16".to_owned(),
            counts(&[("approved", 10), ("declined", 10)]),
        );
        let mix = mix(&weeks);

        assert_eq!(mix.len(), 2);
        assert_eq!(mix[0].week, "2026-11-09", "oldest week first");
        assert_eq!(mix[0].records, 40);
        assert_eq!(mix[0].shares["approved"], 0.75);
        assert_eq!(mix[0].shares["declined"], 0.25);
        assert_eq!(mix[1].shares["approved"], 0.5);
        let total: f64 = mix[1].shares.values().sum();
        assert!((total - 1.0).abs() < 1e-12, "a week's shares sum to one");
    }

    #[test]
    fn an_empty_week_reports_no_records_rather_than_dividing_by_zero() {
        let mut weeks = BTreeMap::new();
        weeks.insert("2026-11-09".to_owned(), BinCounts::new());
        let mix = mix(&weeks);
        assert_eq!(mix[0].records, 0);
        assert!(mix[0].shares.is_empty());
    }
}
