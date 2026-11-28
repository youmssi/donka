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
    assert_eq!(
        b.contribution, 0.0,
        "a bin that did not move contributes nothing"
    );
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
    assert_eq!(
        thresholds.band(0.1),
        Band::Moved,
        "the lower bound is inclusive"
    );
    assert_eq!(thresholds.band(0.2499), Band::Moved);
    assert_eq!(
        thresholds.band(0.25),
        Band::Shifted,
        "the upper bound is inclusive"
    );
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
    let shift = shift(
        "applicant.region",
        &reference,
        &recent,
        10,
        Thresholds::default(),
    );
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
    assert!(
        index > SHIFTED_AT,
        "a vanished outcome is a shift, got {index}"
    );
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
    let all_suppressed = shift(
        "applicant.age",
        &reference,
        &recent,
        3,
        Thresholds::default(),
    );
    assert_eq!(
        all_suppressed.reason,
        Some(Unmeasurable::NoComparableValues)
    );
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
