use super::*;

#[test]
fn outcome_fields_are_dotted_names() {
    for good in ["decision", "result.band", "_x.y_1"] {
        assert!(valid_field_path(good), "{good}");
    }
    for bad in ["", ".", "a.", "1st", "a-b", "a..b", "a b"] {
        assert!(!valid_field_path(bad), "{bad}");
    }
}

fn payload() -> Payload {
    Payload {
        input: json!({
            "applicant": { "nationalId": "CM-1", "income": 150000, "name": "Ada" },
            "loans": [{ "iban": "CM21", "amount": 10 }, { "iban": "CM22", "amount": 20 }]
        }),
        output: Some(json!({ "applicant": { "nationalId": "CM-1" }, "decision": "approve" })),
        error: None,
        trace: Some(json!({
            "table": {
                "name": "limits",
                "input": { "applicant": { "nationalId": "CM-1", "income": 150000 } },
                "output": { "decision": "approve" },
                "traceData": { "reference_map": {
                    "applicant.nationalId": "CM-1",
                    "applicant.nationalId[0]": "C",
                    "applicant.nationalIdType": "passport",
                    "applicant.income": 150000
                } }
            }
        })),
    }
}

#[test]
fn redaction_removes_fields_wherever_the_record_holds_them() {
    let fields = vec!["applicant.nationalId".to_owned(), "loans.iban".to_owned()];
    let redacted = redact(payload(), &fields);
    assert_eq!(
        redacted.input,
        json!({
            "applicant": { "income": 150000, "name": "Ada" },
            "loans": [{ "amount": 10 }, { "amount": 20 }]
        })
    );
    assert_eq!(
        redacted.output,
        Some(json!({ "applicant": {}, "decision": "approve" }))
    );
    let trace = redacted.trace.unwrap();
    assert_eq!(
        trace["table"]["input"],
        json!({ "applicant": { "income": 150000 } })
    );
    assert_eq!(
        trace["table"]["traceData"]["reference_map"],
        json!({ "applicant.nationalIdType": "passport", "applicant.income": 150000 })
    );
    assert!(!serde_json::to_string(&trace).unwrap().contains("CM-1"));
}

#[test]
fn nothing_listed_changes_nothing() {
    let unchanged = redact(payload(), &[]);
    assert_eq!(unchanged.input, payload().input);
    assert_eq!(unchanged.trace, payload().trace);
}

#[test]
fn redacted_fields_are_checked_and_deduplicated() {
    let fields = |list: &[&str]| list.iter().map(|f| (*f).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        normalize_fields(fields(&[" a.b ", "c", "a.b"])).unwrap(),
        fields(&["a.b", "c"])
    );
    assert!(normalize_fields(fields(&["a-b"])).is_err());
    assert!(normalize_fields(fields(&[""])).is_err());
    let many: Vec<String> = (0..=MAX_REDACTED_FIELDS).map(|i| format!("f{i}")).collect();
    assert!(normalize_fields(many).is_err());
}
