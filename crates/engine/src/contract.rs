//! The input contract of a decision (DNK-37): the JSON Schema on its input node.
//!
//! The engine validates every request against that schema (draft-07, which is what the engine
//! uses), so the Runtime enforces the same definition a form is built from. Presentation hints
//! live beside the standard keywords as `x-donka` annotations (labels, help, `pii`), which
//! validation ignores.
//!
//! This module reads the contract out of a graph, checks it is a valid schema, lists the input
//! fields the rules read, and compares two contracts to tell a breaking change from a compatible
//! one.

use bumpalo::Bump;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zen_expression::lexer::Lexer;
use zen_expression::parser::{Node, Parser};

/// The annotation key for Donka's presentation hints inside a property's schema.
pub const HINTS: &str = "x-donka";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ContractError {
    /// The input node's schema is not JSON.
    #[error("the input schema is not valid JSON")]
    NotJson,
    /// The schema is JSON but not a usable JSON Schema.
    #[error("the input schema is not a valid JSON Schema: {0}")]
    InvalidSchema(String),
    /// The contract must describe an object (the request's `context`).
    #[error("the input schema must describe an object (\"type\": \"object\")")]
    NotAnObject,
}

/// The input contract of a graph, or `None` when its input node has no schema.
///
/// The editor stores the schema as a JSON string; an object is accepted too. The engine ignores
/// a schema it cannot parse, so a broken one must be refused here rather than silently dropped.
pub fn input_schema(content: &Value) -> Result<Option<Value>, ContractError> {
    let Some(raw) = input_node(content).and_then(|node| node.pointer("/content/schema")) else {
        return Ok(None);
    };
    let schema = match raw {
        Value::Null => return Ok(None),
        Value::String(text) if text.trim().is_empty() => return Ok(None),
        Value::String(text) => serde_json::from_str(text).map_err(|_| ContractError::NotJson)?,
        Value::Object(_) => raw.clone(),
        _ => return Err(ContractError::NotJson),
    };
    jsonschema::draft7::new(&schema)
        .map_err(|err| ContractError::InvalidSchema(err.to_string()))?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ContractError::NotAnObject);
    }
    Ok(Some(schema))
}

fn input_node(content: &Value) -> Option<&Value> {
    content
        .get("nodes")?
        .as_array()?
        .iter()
        .find(|node| node.get("type").and_then(Value::as_str) == Some("inputNode"))
}

/// One field a contract declares, as a dotted path (`applicant.age`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub path: String,
    /// Required here and in every object above it: a request always carries it.
    pub required: bool,
    /// Personal data: removed from what is sent to be explained.
    pub pii: bool,
    /// The field's own schema (type, limits, allowed values, hints).
    pub schema: Value,
    /// An object whose properties are listed (its fields come after it).
    pub has_properties: bool,
}

/// Every field the contract declares, parents before children, in schema order.
pub fn declared_fields(schema: &Value) -> Vec<Field> {
    let mut fields = Vec::new();
    collect_fields(schema, "", true, &mut fields);
    fields
}

fn collect_fields(schema: &Value, prefix: &str, parent_required: bool, out: &mut Vec<Field>) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for (name, property) in properties {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        let is_required = parent_required && required.contains(name.as_str());
        let has_properties = property.get("properties").is_some_and(Value::is_object);
        out.push(Field {
            path: path.clone(),
            required: is_required,
            pii: property
                .pointer(&format!("/{HINTS}/pii"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            schema: property.clone(),
            has_properties,
        });
        collect_fields(property, &path, is_required, out);
    }
}

/// Paths of the fields marked `pii`.
pub fn pii_fields(schema: &Value) -> Vec<String> {
    declared_fields(schema)
        .into_iter()
        .filter(|field| field.pii)
        .map(|field| field.path)
        .collect()
}

/// What the rules of a graph read from its request.
#[derive(Debug, Default, PartialEq)]
pub struct Reads {
    /// Dotted paths read and not computed anywhere in the graph (or its sub-decisions).
    pub fields: BTreeSet<String>,
    /// A function node reads fields no analysis can see: "nobody reads it" cannot be claimed.
    pub opaque: bool,
}

/// The request fields the graph `key` reads, following its sub-decisions in `decisions`.
pub fn fields_read(decisions: &BTreeMap<String, Value>, key: &str) -> Reads {
    let mut reads = BTreeSet::new();
    let mut produced = BTreeSet::new();
    let mut opaque = false;
    let mut seen = BTreeSet::new();
    walk_graph(
        decisions,
        key,
        &mut seen,
        &mut reads,
        &mut produced,
        &mut opaque,
    );
    let fields = reads
        .into_iter()
        .filter(|path| !produced.iter().any(|made| related(made, path)))
        .collect();
    Reads { fields, opaque }
}

fn walk_graph(
    decisions: &BTreeMap<String, Value>,
    key: &str,
    seen: &mut BTreeSet<String>,
    reads: &mut BTreeSet<String>,
    produced: &mut BTreeSet<String>,
    opaque: &mut bool,
) {
    if !seen.insert(key.to_owned()) {
        return;
    }
    let Some(nodes) = decisions
        .get(key)
        .and_then(|content| content.get("nodes"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for node in nodes {
        let content = node.get("content").unwrap_or(&Value::Null);
        let prefix = content
            .get("outputPath")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty());
        let mut produce = |path: &str| {
            produced.insert(match prefix {
                Some(prefix) => format!("{prefix}.{path}"),
                None => path.to_owned(),
            });
        };
        match node.get("type").and_then(Value::as_str) {
            Some("decisionTableNode") => {
                let inputs = strings(content, "inputs", "id");
                for (id, field) in inputs.iter().zip(strings(content, "inputs", "field")) {
                    standard(&field, reads);
                    for rule in content
                        .get("rules")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(cell) = rule.get(id).and_then(Value::as_str) {
                            unary(cell, reads);
                        }
                    }
                }
                for (id, field) in strings(content, "outputs", "id")
                    .iter()
                    .zip(strings(content, "outputs", "field"))
                {
                    produce(&field);
                    for rule in content
                        .get("rules")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(cell) = rule.get(id).and_then(Value::as_str) {
                            standard(cell, reads);
                        }
                    }
                }
            }
            Some("expressionNode") => {
                for (key, value) in strings(content, "expressions", "key")
                    .into_iter()
                    .zip(strings(content, "expressions", "value"))
                {
                    produce(&key);
                    standard(&value, reads);
                }
            }
            Some("switchNode") => {
                for condition in strings(content, "statements", "condition") {
                    standard(&condition, reads);
                }
            }
            Some("decisionNode") => {
                if let Some(child) = content.get("key").and_then(Value::as_str) {
                    walk_graph(decisions, child, seen, reads, produced, opaque);
                }
                if let Some(prefix) = prefix {
                    produced.insert(prefix.to_owned());
                }
            }
            Some("customNode") => {
                // Connectors: `{{ field }}` placeholders in their request, and their answer.
                let config = content.get("config").unwrap_or(&Value::Null);
                templates(config, reads);
                if let Some(output) = config.get("outputKey").and_then(Value::as_str) {
                    produced.insert(output.to_owned());
                }
            }
            Some("functionNode") => *opaque = true,
            _ => {}
        }
    }
}

/// The string property `field` of each item of the array `list` in `content`.
fn strings(content: &Value, list: &str, field: &str) -> Vec<String> {
    content
        .get(list)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            item.get(field)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        })
        .collect()
}

fn templates(value: &Value, reads: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            let mut rest = text.as_str();
            while let Some(start) = rest.find("{{") {
                let Some(end) = rest[start..].find("}}") else {
                    break;
                };
                standard(&rest[start + 2..start + end], reads);
                rest = &rest[start + end + 2..];
            }
        }
        Value::Array(items) => items.iter().for_each(|item| templates(item, reads)),
        Value::Object(map) => map.values().for_each(|item| templates(item, reads)),
        _ => {}
    }
}

fn standard(source: &str, reads: &mut BTreeSet<String>) {
    parse(source, false, reads);
}

fn unary(source: &str, reads: &mut BTreeSet<String>) {
    parse(source, true, reads);
}

/// Adds the paths an expression reads. An expression that does not parse reads nothing here:
/// the engine reports it when the graph is built.
fn parse(source: &str, is_unary: bool, reads: &mut BTreeSet<String>) {
    let source = source.trim();
    if source.is_empty() {
        return;
    }
    let bump = Bump::new();
    let mut lexer = Lexer::new();
    let Ok(tokens) = lexer.tokenize(&bump, source) else {
        return;
    };
    let Ok(parser) = Parser::try_new(&tokens, &bump) else {
        return;
    };
    let result = if is_unary {
        parser.unary().parse()
    } else {
        parser.standard().parse()
    };
    if result.error().is_ok() {
        collect_paths(result.root, reads);
    }
}

/// `a.b.c` (or `a["b"]`) as a dotted path, when the member chain is that simple.
fn member_path(node: &Node) -> Option<String> {
    match node {
        Node::Identifier(name) => Some((*name).to_owned()),
        Node::Member { node, property } => {
            let base = member_path(node)?;
            match property {
                Node::String(name) => Some(format!("{base}.{name}")),
                _ => Some(base),
            }
        }
        _ => None,
    }
}

fn collect_paths(node: &Node, reads: &mut BTreeSet<String>) {
    if let Some(path) = member_path(node) {
        // `$` is the cell's own column in a unary test, not a request field.
        if path != "$" && !path.starts_with("$.") {
            reads.insert(path);
        }
        if let Node::Member { property, .. } = node {
            if !matches!(property, Node::String(_)) {
                collect_paths(property, reads);
            }
        }
        return;
    }
    let mut each = |child: &Node| collect_paths(child, reads);
    match node {
        Node::TemplateString(parts) | Node::Array(parts) => parts.iter().for_each(|n| each(n)),
        Node::Object(pairs) => pairs.iter().for_each(|(k, v)| {
            each(k);
            each(v);
        }),
        Node::Assignments { list, output } => {
            list.iter().for_each(|(_, v)| each(v));
            if let Some(output) = output {
                each(output);
            }
        }
        Node::Closure { body, .. } | Node::Parenthesized(body) => each(body),
        Node::Member { node, property } => {
            each(node);
            each(property);
        }
        Node::Slice { node, from, to } => {
            each(node);
            from.iter().chain(to.iter()).for_each(|n| each(n));
        }
        Node::Interval { left, right, .. } => {
            each(left);
            each(right);
        }
        Node::Conditional {
            condition,
            on_true,
            on_false,
        } => {
            each(condition);
            each(on_true);
            each(on_false);
        }
        Node::Unary { node, .. } => each(node),
        Node::Binary { left, right, .. } => {
            each(left);
            each(right);
        }
        Node::FunctionCall { arguments, .. } => arguments.iter().for_each(|n| each(n)),
        Node::MethodCall {
            this, arguments, ..
        } => {
            each(this);
            arguments.iter().for_each(|n| each(n));
        }
        _ => {}
    }
}

/// One path is the other, or contains it (`applicant` and `applicant.age`).
fn related(a: &str, b: &str) -> bool {
    a == b || b.starts_with(&format!("{a}.")) || a.starts_with(&format!("{b}."))
}

/// What saving a version should tell its author about the contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Warning {
    /// A rule reads a field the contract does not declare: requests may lack it.
    Undeclared { path: String },
    /// The contract requires a field no rule reads: callers send it for nothing.
    Unread { path: String },
}

/// Compares what the graph `key` reads with its contract. No contract, no warnings.
pub fn check(decisions: &BTreeMap<String, Value>, key: &str, schema: &Value) -> Vec<Warning> {
    let declared = declared_fields(schema);
    let reads = fields_read(decisions, key);
    let mut warnings: Vec<Warning> = reads
        .fields
        .iter()
        .filter(|path| !is_declared(&declared, path))
        .map(|path| Warning::Undeclared { path: path.clone() })
        .collect();
    if !reads.opaque {
        warnings.extend(
            declared
                .iter()
                .filter(|field| field.required && !field.has_properties)
                .filter(|field| !reads.fields.iter().any(|read| related(&field.path, read)))
                .map(|field| Warning::Unread {
                    path: field.path.clone(),
                }),
        );
    }
    warnings
}

/// Declared as such, inside an object the contract leaves open, or a parent of declared fields.
fn is_declared(declared: &[Field], path: &str) -> bool {
    declared.iter().any(|field| {
        field.path == path
            || field.path.starts_with(&format!("{path}."))
            || (!field.has_properties
                && field.schema.get("type").and_then(Value::as_str) == Some("object")
                && path.starts_with(&format!("{}.", field.path)))
    })
}

/// One difference between two versions of a contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
    /// Callers that worked with the old contract may be refused by the new one.
    pub breaking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Removed,
    NowRequired,
    NowOptional,
    TypeChanged,
    /// Fewer values accepted: a tighter limit, fewer allowed values, a new pattern or format.
    Narrowed,
    /// More values accepted.
    Widened,
    /// Labels, help or the `pii` mark: what callers send is unchanged.
    Presentation,
}

/// How the contract changed from `before` to `after` (either may be absent).
pub fn diff(before: Option<&Value>, after: Option<&Value>) -> Vec<Change> {
    let old: BTreeMap<String, Field> = before
        .map(declared_fields)
        .unwrap_or_default()
        .into_iter()
        .map(|field| (field.path.clone(), field))
        .collect();
    let new: BTreeMap<String, Field> = after
        .map(declared_fields)
        .unwrap_or_default()
        .into_iter()
        .map(|field| (field.path.clone(), field))
        .collect();
    let mut changes = Vec::new();
    let mut push = |path: &str, kind: ChangeKind, breaking: bool| {
        changes.push(Change {
            path: path.to_owned(),
            kind,
            breaking,
        });
    };
    for path in old.keys().filter(|path| !new.contains_key(*path)) {
        push(path, ChangeKind::Removed, true);
    }
    for (path, field) in &new {
        let Some(was) = old.get(path) else {
            push(path, ChangeKind::Added, field.required);
            continue;
        };
        if field.required && !was.required {
            push(path, ChangeKind::NowRequired, true);
        } else if !field.required && was.required {
            push(path, ChangeKind::NowOptional, false);
        }
        if was.schema.get("type") != field.schema.get("type") {
            push(path, ChangeKind::TypeChanged, true);
            continue;
        }
        let (narrower, wider) = compare_limits(&was.schema, &field.schema);
        if narrower {
            push(path, ChangeKind::Narrowed, true);
        } else if wider {
            push(path, ChangeKind::Widened, false);
        }
        if was.schema.get(HINTS) != field.schema.get(HINTS)
            || was.schema.get("title") != field.schema.get("title")
            || was.schema.get("description") != field.schema.get("description")
        {
            push(path, ChangeKind::Presentation, false);
        }
    }
    changes
}

/// Whether the new limits accept fewer values, more values, or both.
fn compare_limits(old: &Value, new: &Value) -> (bool, bool) {
    let mut narrower = false;
    let mut wider = false;
    let number = |schema: &Value, key: &str| schema.get(key).and_then(Value::as_f64);
    // Lower bounds: a higher one is narrower.
    for key in ["minimum", "exclusiveMinimum", "minLength", "minItems"] {
        match (number(old, key), number(new, key)) {
            (None, Some(_)) => narrower = true,
            (Some(_), None) => wider = true,
            (Some(a), Some(b)) if b > a => narrower = true,
            (Some(a), Some(b)) if b < a => wider = true,
            _ => {}
        }
    }
    // Upper bounds: a lower one is narrower.
    for key in ["maximum", "exclusiveMaximum", "maxLength", "maxItems"] {
        match (number(old, key), number(new, key)) {
            (None, Some(_)) => narrower = true,
            (Some(_), None) => wider = true,
            (Some(a), Some(b)) if b < a => narrower = true,
            (Some(a), Some(b)) if b > a => wider = true,
            _ => {}
        }
    }
    match (old.get("enum"), new.get("enum")) {
        (None, Some(_)) => narrower = true,
        (Some(_), None) => wider = true,
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            if a.iter().any(|value| !b.contains(value)) {
                narrower = true;
            }
            if b.iter().any(|value| !a.contains(value)) {
                wider = true;
            }
        }
        _ => {}
    }
    for key in ["pattern", "format", "const"] {
        match (old.get(key), new.get(key)) {
            (a, b) if a == b => {}
            (Some(_), None) => wider = true,
            _ => narrower = true,
        }
    }
    (narrower, wider)
}

#[cfg(test)]
mod tests;
