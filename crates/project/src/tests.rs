use super::*;

#[test]
fn roles_include_the_ones_below() {
    assert!(Role::Owner.includes(Role::Editor));
    assert!(Role::Editor.includes(Role::Viewer));
    assert!(!Role::Viewer.includes(Role::Editor));
    assert!(!Role::Editor.includes(Role::Owner));
}

#[test]
fn keys_are_url_safe() {
    for good in ["credit-scoring", "sme2", "ab", "a-1-b"] {
        assert!(check_key(good).is_ok(), "{good}");
    }
    let too_long = "a".repeat(41);
    for bad in [
        "a",
        "Credit",
        "1credit",
        "credit_scoring",
        "credit--scoring",
        "credit-",
        "-credit",
        "crédit",
        "credit scoring",
        too_long.as_str(),
    ] {
        assert!(check_key(bad).is_err(), "{bad}");
    }
}

#[test]
fn names_are_trimmed_and_bounded() {
    assert_eq!(check_name("  Retail scoring ").unwrap(), "Retail scoring");
    assert!(check_name("   ").is_err());
    assert!(check_name(&"é".repeat(100)).is_ok());
    assert!(check_name(&"é".repeat(101)).is_err());
    assert!(check_description(&"x".repeat(1001)).is_err());
}
