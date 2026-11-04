use super::*;

const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

#[test]
fn an_identifier_hashes_the_same_however_it_is_written() {
    let key = PersonKey::from_base64(KEY).unwrap();
    let hash = key.hash("CM-1234.567 ab").unwrap();
    assert_eq!(key.hash(" cm1234567AB ").unwrap(), hash);
    assert_ne!(key.hash("cm1234567ac").unwrap(), hash);
    assert_eq!(key.hash(" -. "), None);
    let other = PersonKey::from_base64("ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=").unwrap();
    assert_ne!(other.hash("cm1234567ab").unwrap(), hash);
    assert!(PersonKey::from_base64("too-short").is_err());
}

#[test]
fn identifiers_are_read_through_lists_and_numbers() {
    let key = PersonKey::from_base64(KEY).unwrap();
    let input = json!({
        "applicant": { "nationalId": "A1", "phone": 237_650_000_000_u64 },
        "guarantors": [{ "nationalId": "B2" }, { "nationalId": null }]
    });
    let paths = vec![
        "applicant.nationalId".to_owned(),
        "applicant.phone".to_owned(),
        "guarantors.nationalId".to_owned(),
        "missing.field".to_owned(),
    ];
    let hashes = key.hashes(&input, &paths);
    let expected: BTreeSet<Vec<u8>> = ["a1", "237650000000", "b2"]
        .iter()
        .map(|value| key.hash(value).unwrap())
        .collect();
    assert_eq!(hashes, expected);
}
