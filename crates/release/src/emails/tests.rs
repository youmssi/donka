use super::*;

#[test]
fn names_the_release_and_links_to_the_review() {
    for language in [Language::En, Language::Fr] {
        let email = approval_requested(language, "Crédit PME", "1.2.0", "https://x/a");
        assert!(email.subject.contains("1.2.0") && email.subject.contains("Crédit PME"));
        assert!(email.body.contains("https://x/a"));
    }
}
