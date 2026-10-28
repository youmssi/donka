//! What is wrong with a graph's decision tables (DNK-76): inputs no row matches (gaps), rows an
//! earlier row already answers (overlaps) and conditions that can never match.
//!
//! The engine's own verification does the analysis; this module keeps the table findings, names
//! their rows and columns the way the editor shows them, and gives each one a fingerprint that
//! stays the same while the finding does, so an acknowledgement outlives the next save.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zen_engine::model::DecisionContent;
use zen_engine::policy::{Diagnostic, DiagnosticCode};
use zen_engine::Workspace;

/// The document name the graph is analysed under; findings never show it.
const DOCUMENT: &str = "decision";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingKind {
    /// Inputs no row matches: the table gives no answer for them.
    Gap,
    /// A row that never fires because an earlier row (or rows) already matches all it matches.
    Overlap,
    /// A condition that no input can satisfy: its row never fires.
    NeverMatches,
    /// The table is too large to check completely; what was found is still reported.
    Incomplete,
}

impl FindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingKind::Gap => "gap",
            FindingKind::Overlap => "overlap",
            FindingKind::NeverMatches => "neverMatches",
            FindingKind::Incomplete => "incomplete",
        }
    }
}

/// A table column: its id in the graph and the name the editor shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Column {
    pub id: String,
    pub name: String,
}

/// One kind of input no row matches: a condition per column, and an input that falls in it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GapCase {
    pub conditions: Vec<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Condition {
    pub column: Column,
    pub condition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableFinding {
    /// Stable while the finding is the same; acknowledgements are kept against it.
    pub fingerprint: String,
    pub kind: FindingKind,
    /// The decision table node.
    pub table_id: String,
    pub table_name: String,
    /// The row at fault, numbered from 1 as in the editor (overlap, never matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_id: Option<String>,
    /// The column at fault (never matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<Column>,
    /// Overlap: the earlier rows that already match everything the row matches.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub covered_by: Vec<u32>,
    /// Overlap: other rows further down that never fire for the same reason.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<u32>,
    /// Overlap: the covering row gives the same result, so nothing changes if it is removed.
    pub same_result: bool,
    /// Gap: the uncovered inputs, the first few of `case_count`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<GapCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_count: Option<u32>,
    /// Overlap: an input the row matches but never gets to answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Value>,
}

/// The table findings of a graph, in table then row order. A document that is not a graph, or
/// that the engine cannot read, has none: saving it reports its own errors elsewhere.
pub fn check(content: &Value) -> Vec<TableFinding> {
    let Ok(document) = serde_json::from_value::<DecisionContent>(content.clone()) else {
        return Vec::new();
    };
    let tables = Tables::read(content);
    if tables.0.is_empty() {
        return Vec::new();
    }
    let mut workspace = Workspace::new();
    workspace.set_document(DOCUMENT, document);
    let mut findings: Vec<TableFinding> = workspace
        .diagnostics(DOCUMENT)
        .iter()
        .filter_map(|diagnostic| tables.finding(diagnostic))
        .collect();
    findings.sort_by(|a, b| {
        let order = |f: &TableFinding| tables.position(&f.table_id);
        order(a)
            .cmp(&order(b))
            .then(a.row.unwrap_or(0).cmp(&b.row.unwrap_or(0)))
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });
    findings.dedup_by(|a, b| a.fingerprint == b.fingerprint);
    findings
}

/// The graph's decision tables, by node id, in the order they appear.
struct Tables(Vec<Table>);

struct Table {
    id: String,
    name: String,
    /// Column ids with the name the editor shows (its name, else its field), in table order.
    columns: Vec<(String, String)>,
    /// Row ids in order.
    rows: Vec<String>,
}

impl Tables {
    fn read(content: &Value) -> Self {
        let nodes = content.get("nodes").and_then(Value::as_array);
        let tables = nodes
            .into_iter()
            .flatten()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("decisionTableNode"))
            .filter_map(|node| {
                let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_owned);
                let table = node.get("content")?;
                let columns = ["inputs", "outputs"]
                    .iter()
                    .filter_map(|key| table.get(*key).and_then(Value::as_array))
                    .flatten()
                    .filter_map(|column| {
                        let id = text(column.get("id"))?;
                        let name = text(column.get("name"))
                            .filter(|name| !name.trim().is_empty())
                            .or_else(|| text(column.get("field")))
                            .unwrap_or_else(|| id.clone());
                        Some((id, name))
                    })
                    .collect();
                let rows = table
                    .get("rules")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|rule| text(rule.get("_id")))
                    .collect();
                Some(Table {
                    id: text(node.get("id"))?,
                    name: text(node.get("name")).unwrap_or_default(),
                    columns,
                    rows,
                })
            })
            .collect();
        Tables(tables)
    }

    fn position(&self, id: &str) -> usize {
        self.0
            .iter()
            .position(|table| table.id == id)
            .unwrap_or(usize::MAX)
    }

    fn finding(&self, diagnostic: &Diagnostic) -> Option<TableFinding> {
        let kind = match diagnostic.code {
            DiagnosticCode::MissingCases => FindingKind::Gap,
            DiagnosticCode::UnreachableRule | DiagnosticCode::DuplicateRule => FindingKind::Overlap,
            DiagnosticCode::UnsatisfiableCell => FindingKind::NeverMatches,
            DiagnosticCode::TableChecksIncomplete => FindingKind::Incomplete,
            _ => return None,
        };
        let table_id = diagnostic.location.block_id.as_deref()?;
        let table = self.0.iter().find(|table| table.id == table_id)?;
        let arg = |key: &str| diagnostic.args.get(key).map(String::as_str);
        let row_id = arg("rowId").map(str::to_owned);
        let row = row_id.as_deref().and_then(|id| table.number(id));
        let ids = |key: &str| -> Vec<&str> {
            arg(key)
                .map(|ids| ids.split(',').filter(|id| !id.is_empty()).collect())
                .unwrap_or_default()
        };
        let covered_ids = match diagnostic.code {
            DiagnosticCode::DuplicateRule => ids("duplicateOfId"),
            _ => ids("coveredByIds"),
        };
        let also_ids: Vec<&str> = ids("blockedIds")
            .into_iter()
            .filter(|id| Some(*id) != row_id.as_deref())
            .collect();
        let column = match kind {
            FindingKind::NeverMatches => diagnostic
                .location
                .expression_id
                .as_deref()
                .map(|id| table.column(id)),
            _ => None,
        };
        let cases = match kind {
            FindingKind::Gap => table.cases(arg("cases")),
            _ => Vec::new(),
        };
        let example = match kind {
            FindingKind::Overlap => arg("example").and_then(|text| serde_json::from_str(text).ok()),
            _ => None,
        };
        let fingerprint = fingerprint(&[
            kind.as_str(),
            table_id,
            row_id.as_deref().unwrap_or(""),
            column.as_ref().map_or("", |column| column.id.as_str()),
            &covered_ids.join(","),
            &serde_json::to_string(&cases_key(&cases)).unwrap_or_default(),
        ]);
        Some(TableFinding {
            fingerprint,
            kind,
            table_id: table.id.clone(),
            table_name: table.name.clone(),
            row,
            row_id,
            column,
            covered_by: covered_ids
                .iter()
                .filter_map(|id| table.number(id))
                .collect(),
            also: also_ids.iter().filter_map(|id| table.number(id)).collect(),
            same_result: arg("redundant") == Some("true"),
            case_count: match kind {
                FindingKind::Gap => arg("count").and_then(|count| count.parse().ok()),
                _ => None,
            },
            cases,
            example,
        })
    }
}

impl Table {
    fn number(&self, row_id: &str) -> Option<u32> {
        let index = self.rows.iter().position(|id| id == row_id)?;
        u32::try_from(index + 1).ok()
    }

    fn column(&self, id: &str) -> Column {
        Column {
            id: id.to_owned(),
            name: self
                .columns
                .iter()
                .find(|(key, _)| key == id)
                .map_or_else(|| id.to_owned(), |(_, name)| name.clone()),
        }
    }

    /// The engine lists each case as `{ cells: { column id: condition }, example }`.
    fn cases(&self, text: Option<&str>) -> Vec<GapCase> {
        let cases: Vec<Value> = text
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        cases
            .iter()
            .map(|case| {
                let cells = case.get("cells").and_then(Value::as_object);
                // Conditions in the table's column order, as the editor shows them.
                let mut conditions: Vec<(usize, Condition)> = cells
                    .into_iter()
                    .flatten()
                    .filter_map(|(id, condition)| {
                        let position = self.columns.iter().position(|(key, _)| key == id);
                        Some((
                            position.unwrap_or(usize::MAX),
                            Condition {
                                column: self.column(id),
                                condition: condition.as_str()?.to_owned(),
                            },
                        ))
                    })
                    .collect();
                conditions.sort_by_key(|(position, _)| *position);
                GapCase {
                    conditions: conditions.into_iter().map(|(_, c)| c).collect(),
                    example: case.get("example").cloned(),
                }
            })
            .collect()
    }
}

/// What identifies a gap: its conditions, not the example the engine picked.
fn cases_key(cases: &[GapCase]) -> Vec<Vec<(&str, &str)>> {
    cases
        .iter()
        .map(|case| {
            case.conditions
                .iter()
                .map(|c| (c.column.id.as_str(), c.condition.as_str()))
                .collect()
        })
        .collect()
}

fn fingerprint(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    hash.finalize()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "tables/tests.rs"]
mod tests;
