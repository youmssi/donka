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
