use super::*;
use crate::stability::Thresholds;

fn snapshot_with(worst: Band, shifts: Vec<Shift>) -> Snapshot {
    Snapshot {
        id: Uuid::nil(),
        decision_key: "scorecard".to_owned(),
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
    assert!(
        en.body.contains("0.420"),
        "the index is quoted: {}",
        en.body
    );
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
