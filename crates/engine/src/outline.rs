//! A decision's content as something a person can read (DNK-79).
//!
//! The graph is the policy, but a reviewer cannot audit JSON. This turns a decision's content
//! into tables with their rows and expressions with their text, and **names every other node
//! kind rather than dropping it**: a document that silently omits part of the policy is worse
//! than one that says what it does not render.
//!
//! Only the graph's shape is read here. `tables` reads the same graph for a different purpose
//! — gaps and overlaps — and shares this module's rule for what a column is called.

use serde_json::Value;

/// One node of a decision, as a document shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Table(Table),
    Expression(Expression),
    /// A node this outline does not render, named with its kind so a reader knows it is there.
    Other {
        name: String,
        kind: String,
    },
}

/// A decision table: its columns and every row, in the order the editor shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    /// `first` or `collect`, as the table declares it.
    pub hit_policy: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// Each row's cells, inputs then outputs, in column order. An empty cell reads `—`,
    /// because a blank in a condition column means "anything" and must not look like a gap.
    pub rows: Vec<Vec<String>>,
}

/// An expression node: each field and the expression that produces it.
#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// An empty cell, in a table or an expression.
const EMPTY: &str = "—";

/// What a column is called: its name, else the field it writes, else its identifier. Shared
/// with `tables` so a column is never named one way in a finding and another in a document.
pub(crate) fn column_label(column: &Value) -> Option<(String, String)> {
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_owned);
    let id = text(column.get("id"))?;
    let name = text(column.get("name"))
        .filter(|name| !name.trim().is_empty())
        .or_else(|| text(column.get("field")))
        .unwrap_or_else(|| id.clone());
    Some((id, name))
}

/// The decision's nodes in the order they appear, input and output nodes left out: they carry
/// no policy, and the contract is shown on its own.
pub fn outline(content: &Value) -> Vec<Node> {
    let Some(nodes) = content.get("nodes").and_then(Value::as_array) else {
        return Vec::new();
    };
    nodes
        .iter()
        .filter_map(|node| {
            let name = node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            match node.get("type").and_then(Value::as_str) {
                Some("inputNode") | Some("outputNode") => None,
                // A table whose content is missing is still named: nothing is dropped.
                Some("decisionTableNode") => Some(match node.get("content") {
                    Some(content) => table(name, content),
                    None => Node::Other {
                        name,
                        kind: "decisionTableNode".to_owned(),
                    },
                }),
                Some("expressionNode") => Some(expression(name, node.get("content"))),
                other => Some(Node::Other {
                    name,
                    kind: kind(node, other.unwrap_or("unknown")),
                }),
            }
        })
        .collect()
}

/// A custom node names what it does; anything else is named by its node type.
fn kind(node: &Value, node_type: &str) -> String {
    node.pointer("/content/kind")
        .and_then(Value::as_str)
        .unwrap_or(node_type)
        .to_owned()
}

fn table(name: String, content: &Value) -> Node {
    let columns = |key: &str| -> Vec<(String, String)> {
        content
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(column_label)
            .collect()
    };
    let inputs = columns("inputs");
    let outputs = columns("outputs");
    let rows = content
        .get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|rule| {
            inputs
                .iter()
                .chain(outputs.iter())
                .map(|(id, _)| cell(rule.get(id)))
                .collect()
        })
        .collect();
    Node::Table(Table {
        name,
        hit_policy: content
            .get("hitPolicy")
            .and_then(Value::as_str)
            .unwrap_or("first")
            .to_owned(),
        inputs: inputs.into_iter().map(|(_, name)| name).collect(),
        outputs: outputs.into_iter().map(|(_, name)| name).collect(),
        rows,
    })
}

fn expression(name: String, content: Option<&Value>) -> Node {
    let entries = content
        .and_then(|content| content.get("expressions"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let key = entry
                .get("key")
                .and_then(Value::as_str)
                .filter(|key| !key.trim().is_empty())?;
            Some((key.to_owned(), cell(entry.get("value"))))
        })
        .collect();
    Node::Expression(Expression { name, entries })
}

/// A cell as text. A blank reads as `—` so a row never looks shorter than it is.
fn cell(value: Option<&Value>) -> String {
    match value.and_then(Value::as_str) {
        Some(text) if !text.trim().is_empty() => text.trim().to_owned(),
        _ => EMPTY.to_owned(),
    }
}

#[cfg(test)]
mod tests;
