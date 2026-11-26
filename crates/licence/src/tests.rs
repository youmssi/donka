use super::*;
use chrono::TimeZone as _;

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn key(byte: u8) -> SigningKeyPair {
    SigningKeyPair::from_seed([byte; 32])
}

fn licence() -> Licence {
    Licence {
        id: Uuid::nil(),
        customer: "Bank of Example".into(),
        issued_at: Utc.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap(),
        valid_from: day("2026-11-01"),
        valid_until: day("2027-10-31"),
        modules: BTreeSet::from([Module::Explain, Module::Connectors]),
    }
}

/// Replaces a field of the signed payload and re-encodes it, keeping the old signature.
fn tamper(file: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut envelope: serde_json::Value = serde_json::from_str(file).unwrap();
    let payload = B64.decode(envelope["payload"].as_str().unwrap()).unwrap();
    let mut payload: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    change(&mut payload);
    envelope["payload"] = B64.encode(serde_json::to_vec(&payload).unwrap()).into();
    envelope.to_string()
}

#[test]
fn a_licence_signed_by_the_trusted_key_reads_back_whole() {
    let signer = key(1);
    let file = signer.issue(&licence()).unwrap();
    assert_eq!(signer.public_key().verify(&file), Ok(licence()));
}

#[test]
fn keys_round_trip_through_their_files() {
    let signer = key(2);
    let again = SigningKeyPair::from_text(&signer.to_text()).unwrap();
    let public = PublicKey::from_text(&signer.public_key().to_text()).unwrap();
    assert_eq!(again.public_key().id(), signer.public_key().id());
    assert_eq!(public.id(), signer.public_key().id());
    assert_eq!(public.id().len(), 16);
    let file = again.issue(&licence()).unwrap();
    assert!(public.verify(&file).is_ok());
}

#[test]
fn a_changed_licence_is_tampered() {
    let signer = key(3);
    let file = signer.issue(&licence()).unwrap();
    let later = tamper(&file, |payload| payload["validUntil"] = "2099-12-31".into());
    assert_eq!(
        signer.public_key().verify(&later),
        Err(LicenceError::Tampered)
    );
    let more = tamper(&file, |payload| {
        payload["modules"] = serde_json::json!(["explain", "connectors", "analysis"]);
    });
    assert_eq!(
        signer.public_key().verify(&more),
        Err(LicenceError::Tampered)
    );
}

#[test]
fn a_licence_from_another_key_is_foreign() {
    let file = key(4).issue(&licence()).unwrap();
    let trusted = key(5).public_key();
    assert!(matches!(
        trusted.verify(&file),
        Err(LicenceError::Foreign { expected, .. }) if expected == trusted.id()
    ));
}

#[test]
fn a_foreign_key_cannot_pass_for_the_trusted_one() {
    // Writing the trusted key's id into a licence signed by another key does not help.
    let trusted = key(6).public_key();
    let file = key(7).issue(&licence()).unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(&file).unwrap();
    envelope["keyId"] = trusted.id().into();
    assert_eq!(
        trusted.verify(&envelope.to_string()),
        Err(LicenceError::Tampered)
    );
}

#[test]
fn files_that_are_not_licences_are_unreadable() {
    let trusted = key(8).public_key();
    for text in [
        "",
        "not json",
        "{}",
        r#"{"format":"donka-licence/2","keyId":"x","payload":"","signature":""}"#,
    ] {
        assert!(
            matches!(trusted.verify(text), Err(LicenceError::Unreadable(_))),
            "{text}"
        );
    }
}

#[test]
fn modules_this_studio_does_not_know_are_ignored() {
    let signer = key(9);
    let mut file: serde_json::Value =
        serde_json::from_str(&signer.issue(&licence()).unwrap()).unwrap();
    // Re-sign a payload naming a module from a later Studio.
    let payload = serde_json::json!({
        "id": Uuid::nil(),
        "customer": "Bank of Example",
        "issuedAt": "2026-10-01T09:00:00Z",
        "validFrom": "2026-11-01",
        "validUntil": "2027-10-31",
        "modules": ["explain", "fieldkit"],
    });
    let bytes = serde_json::to_vec(&payload).unwrap();
    file["payload"] = B64.encode(&bytes).into();
    file["signature"] = B64.encode(signer.key.sign(&bytes).to_bytes()).into();
    let read = signer.public_key().verify(&file.to_string()).unwrap();
    assert_eq!(read.modules, BTreeSet::from([Module::Explain]));
}

#[test]
fn a_licence_needs_a_customer_and_dates_in_order() {
    let signer = key(10);
    let nameless = Licence {
        customer: "  ".into(),
        ..licence()
    };
    assert!(matches!(
        signer.issue(&nameless),
        Err(LicenceError::Invalid(_))
    ));
    let backwards = Licence {
        valid_until: day("2026-10-01"),
        ..licence()
    };
    assert!(matches!(
        signer.issue(&backwards),
        Err(LicenceError::Invalid(_))
    ));
}

#[test]
fn bad_key_files_are_refused() {
    assert!(matches!(
        PublicKey::from_text("not base64!"),
        Err(LicenceError::InvalidKey(_))
    ));
    assert!(matches!(
        PublicKey::from_text(&B64.encode([1u8; 16])),
        Err(LicenceError::InvalidKey(_))
    ));
}

#[test]
fn the_built_in_key_is_readable() {
    let key = PublicKey::built_in().unwrap();
    assert_eq!(key.id().len(), 16);
}

#[test]
fn the_status_follows_the_days() {
    let status = |today: &str| Status::on(Some(Ok(licence())), day(today));
    assert!(matches!(status("2026-10-31"), Status::NotYetValid { .. }));
    assert!(matches!(
        status("2026-11-01"),
        Status::Active { days_left: 364, .. }
    ));
    assert!(matches!(
        status("2027-10-31"),
        Status::Active { days_left: 0, .. }
    ));
    assert!(matches!(status("2027-11-01"), Status::Expired { .. }));
    assert_eq!(Status::on(None, day("2027-01-01")), Status::Evaluation);
}

#[test]
fn administrators_are_told_thirty_days_before_the_end() {
    let status = |today: &str| Status::on(Some(Ok(licence())), day(today));
    assert!(!status("2027-09-30").expiring());
    assert!(status("2027-10-01").expiring());
    assert!(status("2027-10-31").expiring());
    assert!(!status("2027-11-01").expiring());
}

#[test]
fn what_each_status_allows() {
    let today = day("2027-01-01");
    let active = Status::on(Some(Ok(licence())), today);
    assert!(active.allows_changes() && active.allows_production());
    assert!(active.includes(Module::Explain) && !active.includes(Module::Analysis));

    let evaluation = Status::Evaluation;
    assert!(evaluation.allows_changes() && !evaluation.allows_production());
    assert!(Module::ALL.into_iter().all(|m| evaluation.includes(m)));

    let expired = Status::on(Some(Ok(licence())), day("2028-01-01"));
    assert!(!expired.allows_changes() && expired.allows_production());
    assert!(expired.includes(Module::Connectors));

    let refused = Status::on(Some(Err(LicenceError::Tampered)), today);
    assert!(!refused.allows_changes() && refused.allows_production());
    assert!(Module::ALL.into_iter().all(|m| !refused.includes(m)));
}
