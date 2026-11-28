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
    assert_eq!(
        bins.label(500.0),
        "<600",
        "an edge belongs to the range above it"
    );
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
    assert_eq!(
        at_path(&payload, "score.deeper"),
        None,
        "a number has no fields"
    );
}

#[test]
fn an_absent_or_null_field_bins_as_missing() {
    let characteristic = by_value("applicant.region");
    assert_eq!(bin_of(None, &characteristic), "missing");
    assert_eq!(bin_of(Some(&json!({})), &characteristic), "missing");
    assert_eq!(
        bin_of(
            Some(&json!({"applicant": {"region": null}})),
            &characteristic
        ),
        "missing",
        "a field that stopped being sent shows as a shift, not as an absence"
    );
}

#[test]
fn a_value_bin_keeps_strings_as_they_are_and_renders_the_rest() {
    let characteristic = by_value("applicant.region");
    assert_eq!(
        bin_of(
            Some(&json!({"applicant": {"region": "north"}})),
            &characteristic
        ),
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
    assert_eq!(
        bin_of(Some(&json!({"score": 550})), &characteristic),
        "<600"
    );
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
    assert_eq!(
        week_of(sunday),
        "2026-11-23",
        "a Sunday closes the same week"
    );
    let next: DateTime<Utc> = "2026-11-30T00:00:00Z".parse().unwrap();
    assert_eq!(week_of(next), "2026-11-30", "Monday starts the next one");
}
