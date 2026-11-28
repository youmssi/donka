#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use serde_json::json;

fn graph(nodes: Value) -> Value {
    json!({ "nodes": nodes, "edges": [] })
}

#[test]
fn a_table_reads_as_its_columns_and_rows_in_order() {
    let content = graph(json!([
        { "id": "in", "type": "inputNode", "name": "Request" },
        {
            "id": "t", "type": "decisionTableNode", "name": "Limit",
            "content": {
                "hitPolicy": "first",
                "inputs": [{ "id": "c1", "name": "Income", "field": "income" }],
                "outputs": [{ "id": "c2", "field": "limit" }],
                "rules": [
                    { "_id": "r1", "c1": "> 100000", "c2": "500000" },
                    { "_id": "r2", "c1": "", "c2": "0" }
                ]
            }
        },
        { "id": "out", "type": "outputNode", "name": "Response" }
    ]));
    let nodes = outline(&content);
    assert_eq!(nodes.len(), 1, "the input and output nodes carry no policy");
    let Node::Table(table) = &nodes[0] else {
        panic!("expected a table: {nodes:?}");
    };
    assert_eq!(table.name, "Limit");
    assert_eq!(table.hit_policy, "first");
    // A column is named by its name, and by its field when it has no name.
    assert_eq!(table.inputs, ["Income"]);
    assert_eq!(table.outputs, ["limit"]);
    assert_eq!(
        table.rows,
        vec![
            vec!["> 100000".to_owned(), "500000".to_owned()],
            // A blank condition means "anything"; it must not read as a missing cell.
            vec![EMPTY.to_owned(), "0".to_owned()],
        ]
    );
}

#[test]
fn an_expression_reads_as_its_fields_and_their_text() {
    let content = graph(json!([{
        "id": "e", "type": "expressionNode", "name": "Ratio",
        "content": { "expressions": [
            { "id": "1", "key": "dti", "value": "repayment / income" },
            { "id": "2", "key": "", "value": "ignored" },
            { "id": "3", "key": "flag", "value": "  " }
        ] }
    }]));
    let nodes = outline(&content);
    let Node::Expression(expression) = &nodes[0] else {
        panic!("expected an expression: {nodes:?}");
    };
    assert_eq!(expression.name, "Ratio");
    assert_eq!(
        expression.entries,
        vec![
            ("dti".to_owned(), "repayment / income".to_owned()),
            ("flag".to_owned(), EMPTY.to_owned()),
        ],
        "a field with no name has nothing to show; one with no expression is still a field"
    );
}

/// The point of the outline: what it cannot render, it names.
#[test]
fn every_other_node_kind_is_named_rather_than_dropped() {
    let content = graph(json!([
        { "id": "s", "type": "switchNode", "name": "Route" },
        { "id": "f", "type": "functionNode", "name": "Score" },
        { "id": "c", "type": "customNode", "name": "Bureau", "content": { "kind": "donka.connector" } },
        { "id": "d", "type": "decisionNode", "name": "Shared policy" }
    ]));
    let named: Vec<(String, String)> = outline(&content)
        .into_iter()
        .map(|node| match node {
            Node::Other { name, kind } => (name, kind),
            other => panic!("expected a named node: {other:?}"),
        })
        .collect();
    assert_eq!(
        named,
        vec![
            ("Route".to_owned(), "switchNode".to_owned()),
            ("Score".to_owned(), "functionNode".to_owned()),
            // A custom node says what it does, not that it is custom.
            ("Bureau".to_owned(), "donka.connector".to_owned()),
            ("Shared policy".to_owned(), "decisionNode".to_owned()),
        ]
    );
}

#[test]
fn malformed_content_outlines_as_nothing_or_as_a_named_node() {
    assert!(outline(&json!({})).is_empty());
    assert!(outline(&json!({ "nodes": 7 })).is_empty());
    // A table with no content at all is still named, rather than disappearing.
    let bare = graph(json!([{ "id": "t", "type": "decisionTableNode", "name": "Empty" }]));
    assert_eq!(
        outline(&bare),
        vec![Node::Other {
            name: "Empty".to_owned(),
            kind: "decisionTableNode".to_owned()
        }]
    );
}

#[test]
fn a_table_with_no_rows_still_shows_its_columns() {
    let content = graph(json!([{
        "id": "t", "type": "decisionTableNode", "name": "Planned",
        "content": { "inputs": [{ "id": "c1", "field": "age" }], "outputs": [], "rules": [] }
    }]));
    let Node::Table(table) = &outline(&content)[0] else {
        panic!("expected a table");
    };
    assert_eq!(table.inputs, ["age"]);
    assert!(table.rows.is_empty());
    assert_eq!(table.hit_policy, "first", "the editor's default");
}
