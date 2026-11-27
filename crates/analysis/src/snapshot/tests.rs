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
