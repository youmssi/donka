use super::*;

#[test]
fn durations_read_naturally() {
    assert_eq!(describe(Duration::hours(72), Locale::En), "72 hours");
    assert_eq!(describe(Duration::hours(1), Locale::Fr), "1 heure");
    assert_eq!(describe(Duration::minutes(30), Locale::Fr), "30 minutes");
    assert_eq!(describe(Duration::minutes(90), Locale::En), "90 minutes");
}

#[test]
fn every_email_carries_the_link_in_both_languages() {
    for kind in [Kind::Invitation, Kind::PasswordReset] {
        for locale in [Locale::En, Locale::Fr] {
            let content = render(kind, locale, "https://studio/x?token=t", Duration::hours(1));
            assert!(content.text.contains("https://studio/x?token=t"));
            assert!(!content.subject.is_empty());
        }
        assert_eq!(Kind::parse(kind.as_str()), Some(kind));
    }
}

#[test]
fn notices_carry_no_link_in_both_languages() {
    for kind in [
        Notice::SecondStepEnabled,
        Notice::SecondStepRemoved,
        Notice::SecondStepReset,
        Notice::RecoveryCodeUsed,
    ]
    .map(Kind::Notice)
    {
        assert!(!kind.carries_link());
        for locale in [Locale::En, Locale::Fr] {
            let content = render(kind, locale, "https://studio/x?token=t", Duration::hours(1));
            assert!(!content.text.contains("https://"));
            assert!(!content.subject.is_empty());
        }
        assert_eq!(Kind::parse(kind.as_str()), Some(kind));
    }
}
