use super::*;

#[test]
fn tokens_are_unique_and_only_their_hash_matches() {
    let (a, hash_a) = new_token().unwrap();
    let (b, _) = new_token().unwrap();
    assert_ne!(a, b);
    assert_eq!(a.len(), 43);
    assert_eq!(hash_token(&a), hash_a);
    assert_ne!(hash_token(&b), hash_a);
}

#[test]
fn a_password_verifies_only_against_its_own_hash() {
    let hash = hash_password("correct horse battery").unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(verify_password("correct horse battery", Some(&hash)));
    assert!(!verify_password("wrong horse battery", Some(&hash)));
}

#[test]
fn no_stored_hash_never_verifies() {
    assert!(!verify_password("anything at all", None));
}
