use super::*;

#[test]
fn emails_are_normalized_and_validated() {
    assert_eq!(
        normalize_email("  Ada@Bank.EXAMPLE ").unwrap(),
        "ada@bank.example"
    );
    for bad in [
        "",
        "ada",
        "ada@",
        "@bank.example",
        "ada@bank",
        "a da@bank.example",
    ] {
        assert!(normalize_email(bad).is_err(), "{bad}");
    }
}

#[test]
fn password_length_rules() {
    assert!(check_password_rules("short").is_err());
    assert!(check_password_rules(&"x".repeat(12)).is_ok());
    assert!(check_password_rules(&"x".repeat(128)).is_ok());
    assert!(check_password_rules(&"x".repeat(129)).is_err());
}

#[test]
fn retries_back_off_exponentially_up_to_an_hour() {
    assert_eq!(retry_delay(1), Duration::seconds(30));
    assert_eq!(retry_delay(2), Duration::seconds(60));
    assert_eq!(retry_delay(5), Duration::seconds(480));
    assert_eq!(retry_delay(9), Duration::hours(1));
    assert_eq!(retry_delay(i32::MAX), Duration::hours(1));
}

#[test]
fn locales_parse_from_their_codes() {
    assert_eq!("fr".parse::<Locale>().unwrap(), Locale::Fr);
    assert_eq!(Locale::En.as_str(), "en");
    assert!("de".parse::<Locale>().is_err());
}

#[test]
fn issued_tokens_do_not_print_their_value() {
    let token = IssuedToken("secret-value".into());
    assert_eq!(format!("{token:?}"), "IssuedToken(***)");
}
