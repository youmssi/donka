#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use serde_json::json;

/// A graph whose one table rates applicants by age and VIP status.
fn graph(rules: Value) -> Value {
    let schema = json!({
        "type": "object",
        "required": ["applicant"],
        "properties": { "applicant": {
            "type": "object",
            "required": ["age"],
            "properties": { "age": { "type": "number" }, "vip": { "type": "boolean" } }
        }}
    });
    json!({
        "nodes": [
            { "id": "in", "name": "Request", "type": "inputNode", "content": { "schema": schema.to_string() } },
            { "id": "dt", "name": "Rate table", "type": "decisionTableNode", "content": {
                "hitPolicy": "first",
                "inputs": [
                    { "id": "age", "name": "Age", "field": "applicant.age" },
                    { "id": "vip", "name": "", "field": "applicant.vip" }
                ],
                "outputs": [{ "id": "rate", "name": "Rate", "field": "rate" }],
                "rules": rules
            }},
            { "id": "out", "name": "Response", "type": "outputNode", "content": {} }
        ],
        "edges": [
            { "id": "a", "sourceId": "in", "targetId": "dt" },
            { "id": "b", "sourceId": "dt", "targetId": "out" }
        ]
    })
}

fn row(id: &str, age: &str, vip: &str, rate: &str) -> Value {
    json!({ "_id": id, "age": age, "vip": vip, "rate": rate })
}

fn kinds(findings: &[TableFinding]) -> Vec<(FindingKind, Option<u32>)> {
    findings.iter().map(|f| (f.kind, f.row)).collect()
}

#[test]
fn a_complete_table_has_no_findings() {
    let table = graph(json!([
        row("r1", "< 18", "", "0"),
        row("r2", ">= 18", "true", "3"),
        row("r3", ">= 18", "", "5"),
    ]));
    assert_eq!(check(&table), vec![]);
}

#[test]
fn a_gap_names_the_uncovered_inputs_by_column() {
    let findings = check(&graph(json!([
        row("r1", "< 18", "", "0"),
        row("r2", "[18..60]", "true", "3"),
    ])));
    assert_eq!(kinds(&findings), vec![(FindingKind::Gap, None)]);
    let gap = &findings[0];
    assert_eq!(
        (gap.table_id.as_str(), gap.table_name.as_str()),
        ("dt", "Rate table")
    );
    assert_eq!(gap.case_count, Some(2));
    let first = &gap.cases[0];
    assert_eq!(first.conditions[0].column.name, "Age");
    assert_eq!(first.conditions[0].condition, "> 60");
    assert!(first.example.is_some());
    // A column without a name is named by its field; conditions follow the column order.
    let second = &gap.cases[1];
    let columns: Vec<&str> = second
        .conditions
        .iter()
        .map(|c| c.column.name.as_str())
        .collect();
    assert_eq!(columns, vec!["Age", "applicant.vip"]);
}

#[test]
fn a_row_an_earlier_row_answers_is_an_overlap() {
    let findings = check(&graph(json!([
        row("r1", "< 18", "", "1"),
        row("r2", "< 10", "", "2"),
        row("r3", "", "", "5"),
    ])));
    assert_eq!(kinds(&findings), vec![(FindingKind::Overlap, Some(2))]);
    let overlap = &findings[0];
    assert_eq!(overlap.row_id.as_deref(), Some("r2"));
    assert_eq!(overlap.covered_by, vec![1]);
    assert!(!overlap.same_result);
    assert!(overlap.example.is_some());
}

#[test]
fn a_duplicate_row_is_an_overlap_with_the_row_it_repeats() {
    let findings = check(&graph(json!([
        row("r1", "< 18", "", "1"),
        row("r2", ">= 18", "", "2"),
        row("r3", "< 18", "", "1"),
    ])));
    let overlap = findings.iter().find(|f| f.row == Some(3)).unwrap();
    assert_eq!(overlap.kind, FindingKind::Overlap);
    assert_eq!(overlap.covered_by, vec![1]);
    assert!(overlap.same_result);
}

#[test]
fn a_condition_no_input_satisfies_names_its_row_and_column() {
    let findings = check(&graph(json!([
        row("r1", "> 70 and < 50", "", "1"),
        row("r2", "", "", "5"),
    ])));
    assert_eq!(kinds(&findings), vec![(FindingKind::NeverMatches, Some(1))]);
    assert_eq!(
        findings[0].column,
        Some(Column {
            id: "age".into(),
            name: "Age".into()
        })
    );
}

#[test]
fn the_fingerprint_follows_the_finding_not_the_rest_of_the_table() {
    let before = check(&graph(json!([
        row("r1", "< 18", "", "1"),
        row("r2", "< 10", "", "2"),
        row("r3", "", "", "5"),
    ])));
    // Another row's result changes: the overlap is the same finding.
    let after = check(&graph(json!([
        row("r1", "< 18", "", "1"),
        row("r2", "< 10", "", "2"),
        row("r3", "", "", "7"),
    ])));
    assert_eq!(before[0].fingerprint, after[0].fingerprint);
    // A row moves and is covered by another: a different finding.
    let moved = check(&graph(json!([
        row("r0", "< 12", "", "9"),
        row("r1", "< 18", "", "1"),
        row("r2", "< 10", "", "2"),
        row("r3", "", "", "5"),
    ])));
    assert_ne!(before[0].fingerprint, moved[0].fingerprint);
}

#[test]
fn graphs_without_tables_and_other_documents_have_no_findings() {
    assert_eq!(check(&json!({ "nodes": [], "edges": [] })), vec![]);
    assert_eq!(check(&json!("not a graph")), vec![]);
}
