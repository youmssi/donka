use super::*;

#[test]
fn recovery_codes_are_twelve_letters_in_three_groups() {
    let code = new_recovery_code().unwrap();
    assert_eq!(code.len(), 14);
    assert_eq!(code.matches('-').count(), 2);
    assert!(code
        .chars()
        .filter(|c| *c != '-')
        .all(|c| RECOVERY_ALPHABET.contains(&(c as u8))));
    assert_ne!(code, new_recovery_code().unwrap());
}

#[test]
fn recovery_codes_are_read_loosely() {
    assert_eq!(
        normalize_recovery_code(" abcd efgh-jkmn ").as_deref(),
        Some("ABCD-EFGH-JKMN")
    );
    assert_eq!(normalize_recovery_code("123456"), None);
}

#[test]
fn a_recovery_hash_depends_on_the_user() {
    let code = "ABCD-EFGH-JKMN";
    assert_ne!(
        recovery_hash(Uuid::new_v4(), code),
        recovery_hash(Uuid::new_v4(), code)
    );
}

#[test]
fn codes_follow_rfc_6238() {
    // RFC 6238 appendix B, SHA-1 secret, truncated to six digits.
    let totp = totp(b"12345678901234567890".to_vec(), "ada@bank.example").unwrap();
    assert_eq!(totp.generate(59).to_string(), "287082");
    assert_eq!(totp.generate(1_111_111_109).to_string(), "081804");
    assert_eq!(totp.check("287082", 59 + 30), Some(1));
    assert_eq!(totp.check("287082", 59 + 90), None);
    assert!(totp
        .to_url()
        .unwrap()
        .starts_with("otpauth://totp/Donka:ada%40bank.example?"));
}
