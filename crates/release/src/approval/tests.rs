use super::*;

fn row(key: &str, id: u128, version: i32) -> FrozenRow {
    FrozenRow {
        decision_id: Uuid::from_u128(id),
        key: key.into(),
        version_number: version,
        tests: TestSummary::default(),
        content: serde_json::json!({ "nodes": [], "edges": [] }),
    }
}

fn with_contract(mut row: FrozenRow, schema: serde_json::Value) -> FrozenRow {
    row.content = serde_json::json!({ "nodes": [
        { "id": "in", "type": "inputNode", "content": { "schema": schema.to_string() } }
    ]});
    row
}

#[test]
fn contract_changes_come_with_each_decision_breaking_first() {
    let age = |minimum: i64| {
        serde_json::json!({ "type": "object", "required": ["age"], "properties": {
            "age": { "type": "integer", "minimum": minimum },
            "city": { "type": "string" }
        }})
    };
    let mut new = age(21);
    new["properties"]["region"] = serde_json::json!({ "type": "string" });
    let changes = compare(
        &[with_contract(row("score", 1, 1), age(18))],
        &[with_contract(row("score", 1, 2), new)],
    );
    let contract: Vec<(&str, contract::ChangeKind, bool)> = changes[0]
        .contract
        .iter()
        .map(|c| (c.path.as_str(), c.kind, c.breaking))
        .collect();
    assert_eq!(
        contract,
        [
            ("age", contract::ChangeKind::Narrowed, true),
            ("region", contract::ChangeKind::Added, false),
        ]
    );
    // The same version on both sides: nothing to report.
    let same = compare(
        &[with_contract(row("score", 1, 1), age(18))],
        &[with_contract(row("score", 1, 1), age(18))],
    );
    assert!(same[0].contract.is_empty());
}

#[test]
fn compares_production_with_the_release() {
    let from = [row("a", 1, 1), row("b", 2, 3), row("gone", 3, 1)];
    let to = [row("a", 1, 2), row("b", 2, 3), row("new", 4, 1)];
    let changes: Vec<(String, Change, Option<i32>, Option<i32>)> = compare(&from, &to)
        .into_iter()
        .map(|c| (c.key, c.change, c.from_version, c.to_version))
        .collect();
    assert_eq!(
        changes,
        vec![
            ("a".into(), Change::Changed, Some(1), Some(2)),
            ("new".into(), Change::Added, None, Some(1)),
            ("gone".into(), Change::Removed, Some(1), None),
            ("b".into(), Change::Unchanged, Some(3), Some(3)),
        ]
    );
}

#[test]
fn a_renamed_decision_is_the_same_decision() {
    let changes = compare(&[row("old-name", 1, 1)], &[row("new-name", 1, 2)]);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change, Change::Changed);
}

#[test]
fn statuses_read_and_write() {
    for status in [
        ApprovalStatus::Pending,
        ApprovalStatus::Approved,
        ApprovalStatus::Rejected,
        ApprovalStatus::Withdrawn,
    ] {
        assert_eq!(ApprovalStatus::parse(status.as_str()), Some(status));
    }
}
