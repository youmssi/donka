use super::*;

#[test]
fn keys_are_words_and_folders() {
    for good in [
        "person-score",
        "bureau/normalize",
        "a",
        "v2/limits/card-2026",
    ] {
        assert_eq!(check_key(good).unwrap(), good, "{good}");
    }
    for bad in [
        "", "Person", "2fa", "a--b", "a-", "/a", "a/", "a//b", "a b", "a_b", "a/-b",
    ] {
        assert!(check_key(bad).is_err(), "{bad}");
    }
    assert!(check_key(&"a".repeat(MAX_KEY_CHARS + 1)).is_err());
}

#[test]
fn an_empty_graph_is_a_valid_decision() {
    check_content("new", &empty_graph()).unwrap();
    assert!(matches!(
        check_content("broken", &json!({ "nodes": "no" })),
        Err(DecisionError::InvalidContent { .. })
    ));
}
