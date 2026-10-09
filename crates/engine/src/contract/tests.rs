use super::*;
use serde_json::json;

fn pack(name: &str) -> BTreeMap<String, Value> {
    let dir = format!(
        "{}/../../packs/{name}/decisions",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let key = path.file_stem().unwrap().to_str().unwrap().to_owned();
            (
                key,
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap(),
            )
        })
        .collect()
}

fn graph_with_schema(schema: Value) -> Value {
    json!({
        "nodes": [
            { "id": "in", "type": "inputNode", "name": "Request", "content": { "schema": schema } },
            { "id": "out", "type": "outputNode", "name": "Response" }
        ],
        "edges": []
    })
}

fn contract() -> Value {
    json!({
        "type": "object",
        "required": ["applicant"],
        "properties": {
            "applicant": {
                "type": "object",
                "required": ["age", "nationalId"],
                "properties": {
                    "age": { "type": "integer", "minimum": 18, "maximum": 99,
                             "x-donka": { "label": { "en": "Age", "fr": "Âge" } } },
                    "nationalId": { "type": "string", "x-donka": { "pii": true } },
                    "employment": { "type": "string", "enum": ["public", "private"] }
                }
            },
            "loan": {
                "type": "object",
                "properties": { "amount": { "type": "number" } }
            }
        }
    })
}

#[test]
fn the_schema_is_read_from_the_input_node_as_the_editor_stores_it() {
    let text = contract().to_string();
    assert_eq!(
        input_schema(&graph_with_schema(json!(text))).unwrap(),
        Some(contract())
    );
    assert_eq!(
        input_schema(&graph_with_schema(contract())).unwrap(),
        Some(contract())
    );
    assert_eq!(input_schema(&graph_with_schema(json!(""))).unwrap(), None);
    assert_eq!(input_schema(&json!({ "nodes": [] })).unwrap(), None);
}

#[test]
fn a_broken_schema_is_refused_rather_than_ignored() {
    assert_eq!(
        input_schema(&graph_with_schema(json!("{ not json"))),
        Err(ContractError::NotJson)
    );
    assert!(matches!(
        input_schema(&graph_with_schema(
            json!({ "type": "object", "minProperties": "two" })
        )),
        Err(ContractError::InvalidSchema(_))
    ));
    assert_eq!(
        input_schema(&graph_with_schema(json!({ "type": "string" }))),
        Err(ContractError::NotAnObject)
    );
}

#[test]
fn declared_fields_follow_nesting_required_and_pii() {
    let fields = declared_fields(&contract());
    let summary: Vec<(&str, bool, bool)> = fields
        .iter()
        .map(|f| (f.path.as_str(), f.required, f.pii))
        .collect();
    assert_eq!(
        summary,
        [
            ("applicant", true, false),
            ("applicant.age", true, false),
            ("applicant.employment", false, false),
            ("applicant.nationalId", true, true),
            ("loan", false, false),
            ("loan.amount", false, false),
        ]
    );
    assert_eq!(pii_fields(&contract()), ["applicant.nationalId"]);
}

#[test]
fn the_retail_scorecard_reads_its_request_fields_and_not_what_it_computes() {
    let reads = fields_read(&pack("retail-credit"), "scorecard");
    assert!(!reads.opaque);
    let fields: Vec<&str> = reads.fields.iter().map(String::as_str).collect();
    assert_eq!(
        fields,
        [
            "applicant.age",
            "applicant.employment",
            "applicant.monthlyDebtPayments",
            "applicant.monthlyIncome",
            "applicant.monthsInJob",
            "applicant.salaryDomiciled",
            "bureau.activeLoans",
            "bureau.checked",
            "bureau.worstDaysPastDue",
            "loan.amount",
            "loan.termMonths",
        ]
    );
}

#[test]
fn saving_warns_about_undeclared_reads_and_required_fields_nobody_reads() {
    let mut decisions = BTreeMap::new();
    decisions.insert(
        "score".to_owned(),
        json!({
            "nodes": [
                { "id": "in", "type": "inputNode", "name": "Request" },
                { "id": "t", "type": "decisionTableNode", "name": "Age", "content": {
                    "hitPolicy": "first",
                    "inputs": [{ "id": "i", "field": "applicant.age" }],
                    "outputs": [{ "id": "o", "field": "points" }],
                    "rules": [{ "_id": "r", "i": "< limits.minAge", "o": "loan.amount / 1000" }]
                }},
                { "id": "out", "type": "outputNode", "name": "Response" }
            ],
            "edges": []
        }),
    );
    let warnings = check(&decisions, "score", &contract());
    assert_eq!(
        warnings,
        [
            Warning::Undeclared {
                path: "limits.minAge".into()
            },
            Warning::Unread {
                path: "applicant.nationalId".into()
            },
        ]
    );

    // A function node may read anything: no "nobody reads it" claims then.
    decisions.get_mut("score").unwrap()["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "f", "type": "functionNode", "name": "Custom", "content": { "source": "" } }));
    assert_eq!(
        check(&decisions, "score", &contract()),
        [Warning::Undeclared {
            path: "limits.minAge".into()
        }]
    );
}

#[test]
fn connector_placeholders_count_as_reads() {
    let mut decisions = BTreeMap::new();
    decisions.insert(
        "bureau".to_owned(),
        json!({ "nodes": [{ "id": "c", "type": "customNode", "name": "bureau", "content": {
            "kind": "donka.connector",
            "config": { "body": { "id": "{{ applicant.nationalId }}" }, "outputKey": "bureau" }
        }}]}),
    );
    let reads = fields_read(&decisions, "bureau");
    assert_eq!(
        reads.fields.into_iter().collect::<Vec<_>>(),
        ["applicant.nationalId"]
    );
}

#[test]
fn contract_changes_are_marked_breaking_or_compatible() {
    let before = contract();
    let mut after = contract();
    let applicant = &mut after["properties"]["applicant"];
    // Narrower: a higher minimum, one allowed value fewer.
    applicant["properties"]["age"]["minimum"] = json!(21);
    applicant["properties"]["employment"]["enum"] = json!(["public"]);
    // Removed and added (required): breaking.
    applicant["properties"]
        .as_object_mut()
        .unwrap()
        .remove("nationalId");
    applicant["required"] = json!(["age", "income"]);
    applicant["properties"]["income"] = json!({ "type": "number" });
    // Added optional and a new label: compatible.
    after["properties"]["loan"]["properties"]["termMonths"] = json!({ "type": "integer" });
    after["properties"]["loan"]["properties"]["amount"]["x-donka"] =
        json!({ "label": { "en": "Amount" } });

    let changes: Vec<(String, ChangeKind, bool)> = diff(Some(&before), Some(&after))
        .into_iter()
        .map(|c| (c.path, c.kind, c.breaking))
        .collect();
    assert_eq!(
        changes,
        [
            ("applicant.nationalId".into(), ChangeKind::Removed, true),
            ("applicant.age".into(), ChangeKind::Narrowed, true),
            ("applicant.employment".into(), ChangeKind::Narrowed, true),
            ("applicant.income".into(), ChangeKind::Added, true),
            ("loan.amount".into(), ChangeKind::Presentation, false),
            ("loan.termMonths".into(), ChangeKind::Added, false),
        ]
    );

    // The other way round: optional now, wider limits, a type change.
    let mut looser = contract();
    looser["properties"]["applicant"]["required"] = json!(["nationalId"]);
    looser["properties"]["applicant"]["properties"]["age"]["maximum"] = json!(120);
    looser["properties"]["loan"]["properties"]["amount"]["type"] = json!("string");
    let changes: Vec<(String, ChangeKind, bool)> = diff(Some(&before), Some(&looser))
        .into_iter()
        .map(|c| (c.path, c.kind, c.breaking))
        .collect();
    assert_eq!(
        changes,
        [
            ("applicant.age".into(), ChangeKind::NowOptional, false),
            ("applicant.age".into(), ChangeKind::Widened, false),
            ("loan.amount".into(), ChangeKind::TypeChanged, true),
        ]
    );

    // A first contract only adds; dropping the contract removes everything.
    assert!(diff(None, Some(&before))
        .iter()
        .all(|c| c.kind == ChangeKind::Added));
    assert!(diff(Some(&before), None).iter().all(|c| c.breaking));
}
