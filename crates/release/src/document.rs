//! The policy document an auditor reads (DNK-79): a release as a PDF in English or French —
//! its decisions, their tables and expressions, the contract they accept, the versions they
//! froze, the approvals that let them out and the tests that ran on them.
//!
//! What the document says lives here; how a page is laid out lives in `donka_shared::pdf`,
//! which the access report (DNK-75) uses as well. Nothing here reads the database: the caller
//! gathers, so a document can be rendered from any source the modules already expose.

use crate::{Approval, ApprovalStatus, Environment, Release, ReleaseError, Releases};
use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_decision::{TestSummary, VersionSummary};
use donka_engine::contract::{self, Field};
use donka_engine::outline::{outline, Node};
use donka_project::Access;
use donka_shared::language::Language;
use donka_shared::pdf::{date, Pdf, Style};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Why a document could not be produced: the document writer failed.
#[derive(Debug, thiserror::Error)]
#[error("the policy document could not be written: {0}")]
pub struct DocumentError(#[from] donka_shared::pdf::PdfError);

/// What the document says, gathered by the caller.
#[derive(Debug, Clone)]
pub struct PolicyDocument {
    /// The organisation the installation serves (DNK-74), when it is named.
    pub organisation: Option<String>,
    pub project: String,
    pub version: String,
    pub notes: String,
    pub released_by: String,
    pub released_at: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub language: Language,
    /// The environments this release is live in now; empty when it is live nowhere.
    pub live_in: Vec<Environment>,
    pub decisions: Vec<DocumentDecision>,
    pub approvals: Vec<DocumentApproval>,
}

/// One decision as the release froze it.
#[derive(Debug, Clone)]
pub struct DocumentDecision {
    pub key: String,
    pub version: i32,
    pub tests: TestSummary,
    /// Every version of this decision up to the one the release froze, newest first.
    pub history: Vec<DocumentVersion>,
    /// The fields a request carries, from the decision's own contract.
    pub contract: Vec<Field>,
    /// The decision's content, readable.
    pub nodes: Vec<Node>,
}

/// One version of a decision, as its history lists it.
#[derive(Debug, Clone)]
pub struct DocumentVersion {
    pub number: i32,
    pub message: String,
    pub by: String,
    pub at: DateTime<Utc>,
    /// The older version this one restores, when it does.
    pub restored_from: Option<i32>,
}

/// One approval of this release, decided or still waiting.
#[derive(Debug, Clone)]
pub struct DocumentApproval {
    pub status: ApprovalStatus,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    /// Why the release was let out while it reads a protected characteristic (DNK-71).
    pub justification: Option<String>,
    /// Why it was rejected.
    pub reason: Option<String>,
}

struct Words {
    title: &'static str,
    intro: &'static str,
    generated: &'static str,
    released: &'static str,
    live_in: &'static str,
    live_nowhere: &'static str,
    staging: &'static str,
    production: &'static str,
    decision: &'static str,
    version: &'static str,
    no_decisions: &'static str,
    contract: &'static str,
    no_contract: &'static str,
    required: &'static str,
    optional: &'static str,
    protected: &'static str,
    personal: &'static str,
    rules: &'static str,
    hit_first: &'static str,
    hit_collect: &'static str,
    no_rows: &'static str,
    expression: &'static str,
    not_rendered: &'static str,
    history: &'static str,
    restored: &'static str,
    reason: &'static str,
    tests: &'static str,
    no_tests: &'static str,
    passed: &'static str,
    failed: &'static str,
    errors: &'static str,
    approvals: &'static str,
    no_approvals: &'static str,
    requested: &'static str,
    decided: &'static str,
    justification: &'static str,
    pending: &'static str,
    approved: &'static str,
    rejected: &'static str,
    withdrawn: &'static str,
    page: &'static str,
    colon: &'static str,
}

const EN: Words = Words {
    title: "Decision policy",
    intro: "The rules this release holds, as they were approved. Every decision it contains is \
            listed with the contract it accepts, its rules in the order they are read, the \
            versions it froze and the approvals that released it.",
    generated: "written on",
    released: "Released by",
    live_in: "Live in",
    live_nowhere: "This release is not live in any environment.",
    staging: "staging",
    production: "production",
    decision: "Decision",
    version: "version",
    no_decisions: "This release holds no decision.",
    contract: "What a request carries",
    no_contract: "This decision declares no contract: a request may carry anything.",
    required: "required",
    optional: "optional",
    protected: "protected characteristic",
    personal: "personal data",
    rules: "Rules",
    hit_first: "the first row that matches decides",
    hit_collect: "every row that matches is collected",
    no_rows: "This table has no row.",
    expression: "Calculations",
    not_rendered: "This part of the decision is not written out in this document.",
    history: "Version history",
    restored: "restores version",
    reason: "Reason",
    tests: "Tests on the frozen versions",
    no_tests: "No scenario ran on this version.",
    passed: "passed",
    failed: "failed",
    errors: "errors",
    approvals: "Approvals",
    no_approvals: "This release was not sent for approval.",
    requested: "requested by",
    decided: "decided by",
    justification: "Justification",
    pending: "waiting",
    approved: "approved",
    rejected: "rejected",
    withdrawn: "withdrawn",
    page: "Page",
    colon: ": ",
};

const FR: Words = Words {
    title: "Politique de décision",
    intro: "Les règles que contient cette version, telles qu'elles ont été approuvées. Chaque \
            décision est présentée avec le contrat qu'elle accepte, ses règles dans l'ordre où \
            elles sont lues, les versions qu'elle a figées et les approbations qui l'ont libérée.",
    generated: "écrit le",
    released: "Version créée par",
    live_in: "En service dans",
    live_nowhere: "Cette version n'est en service dans aucun environnement.",
    staging: "préproduction",
    production: "production",
    decision: "Décision",
    version: "version",
    no_decisions: "Cette version ne contient aucune décision.",
    contract: "Ce que porte une demande",
    no_contract: "Cette décision ne déclare aucun contrat\u{a0}: une demande peut tout porter.",
    required: "obligatoire",
    optional: "facultatif",
    protected: "caractéristique protégée",
    personal: "donnée personnelle",
    rules: "Règles",
    hit_first: "la première ligne qui correspond décide",
    hit_collect: "toutes les lignes qui correspondent sont retenues",
    no_rows: "Cette table n'a aucune ligne.",
    expression: "Calculs",
    not_rendered: "Cette partie de la décision n'est pas détaillée dans ce document.",
    history: "Historique des versions",
    restored: "restaure la version",
    reason: "Motif",
    tests: "Tests sur les versions figées",
    no_tests: "Aucun scénario n'a été exécuté sur cette version.",
    passed: "réussis",
    failed: "échoués",
    errors: "en erreur",
    approvals: "Approbations",
    no_approvals: "Cette version n'a pas été soumise à approbation.",
    requested: "demandée par",
    decided: "décidée par",
    justification: "Justification",
    pending: "en attente",
    approved: "approuvée",
    rejected: "refusée",
    withdrawn: "retirée",
    page: "Page",
    colon: "\u{a0}: ",
};

impl Releases {
    /// Records that a release was exported as a policy document, and returns the release with
    /// the instant it was exported (any member). The entry states which release, where it was
    /// live and in which language, so a document found in a file can be matched to the export
    /// that made it — and the document is written as of the same instant.
    pub async fn document_exported(
        &self,
        access: &Access,
        id: Uuid,
        language: Language,
    ) -> Result<(Release, DateTime<Utc>), ReleaseError> {
        let release = self.get(access, id).await?;
        let at = self.clock.now();
        let mut tx = self.pool.begin().await?;
        donka_audit::record(
            &mut tx,
            Event::new(at, Some(access.user_id()), Action::ReleaseDocumentExported)
                .in_project(access.project_id())
                .with_details(json!({
                    "releaseId": id,
                    "version": release.summary.version.to_string(),
                    "liveIn": release
                        .summary
                        .live_in
                        .iter()
                        .map(|environment| environment.as_str())
                        .collect::<Vec<_>>(),
                    "language": language.as_str(),
                })),
        )
        .await?;
        tx.commit().await?;
        Ok((release, at))
    }
}

/// What the installation and the reader bring to a document: everything that is not the
/// release itself.
#[derive(Debug, Clone)]
pub struct About {
    /// The organisation the installation serves (DNK-74), when it is named.
    pub organisation: Option<String>,
    pub project: String,
    pub language: Language,
    pub generated_at: DateTime<Utc>,
}

/// The document a release makes, from what the modules already expose: the release and the
/// decisions it froze, their contents, their version histories and the approvals of it.
///
/// `who` turns a user id into something a reader recognises — their email — because this
/// module knows which people a document names but not how to look them up.
pub fn assemble(
    about: About,
    release: &Release,
    contents: &BTreeMap<String, Value>,
    histories: &BTreeMap<String, Vec<VersionSummary>>,
    approvals: &[Approval],
    who: &dyn Fn(Uuid) -> String,
) -> PolicyDocument {
    let summary = &release.summary;
    PolicyDocument {
        organisation: about.organisation,
        project: about.project,
        version: summary.version.to_string(),
        notes: summary.notes.clone(),
        released_by: who(summary.created_by),
        released_at: summary.created_at,
        generated_at: about.generated_at,
        language: about.language,
        live_in: summary.live_in.clone(),
        decisions: release
            .decisions
            .iter()
            .map(|decision| {
                let content = contents.get(&decision.key);
                DocumentDecision {
                    key: decision.key.clone(),
                    version: decision.version,
                    tests: decision.tests,
                    history: histories
                        .get(&decision.key)
                        .map(|versions| {
                            versions
                                .iter()
                                // The history stops at the version this release froze: what
                                // came after it is not in the policy this document describes.
                                .filter(|version| version.number <= decision.version)
                                .map(|version| DocumentVersion {
                                    number: version.number,
                                    message: version.message.clone(),
                                    by: who(version.created_by),
                                    at: version.created_at,
                                    restored_from: version.restored_from,
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    contract: content
                        .and_then(|content| contract::input_schema(content).ok().flatten())
                        .map(|schema| contract::declared_fields(&schema))
                        .unwrap_or_default(),
                    nodes: content.map(outline).unwrap_or_default(),
                }
            })
            .collect(),
        approvals: approvals
            .iter()
            .filter(|approval| approval.release_id == summary.id)
            .map(|approval| DocumentApproval {
                status: approval.status,
                requested_by: who(approval.requested_by),
                requested_at: approval.requested_at,
                decided_by: approval.decided_by.map(who),
                decided_at: approval.decided_at,
                justification: approval.justification.clone(),
                reason: approval.reason.clone(),
            })
            .collect(),
    }
}

/// Who holds the policy, which release it is, and when the document was written.
fn subtitle(document: &PolicyDocument, words: &Words) -> String {
    let holder = match &document.organisation {
        Some(organisation) => format!("{organisation} · {}", document.project),
        None => document.project.clone(),
    };
    format!(
        "{holder} {} — {} {}",
        document.version,
        words.generated,
        date(document.generated_at, document.language)
    )
}

/// The release as a PDF document.
pub fn render(document: &PolicyDocument) -> Result<Vec<u8>, DocumentError> {
    let words = match document.language {
        Language::En => &EN,
        Language::Fr => &FR,
    };
    let mut pdf = Pdf::new(words.page)?;
    pdf.text(Style::Title, words.title);
    pdf.text(Style::Muted, &subtitle(document, words));
    pdf.gap(6.0);
    pdf.text(Style::Body, words.intro);
    pdf.gap(8.0);
    pdf.text(
        Style::Muted,
        &format!(
            "{}{}{} — {}",
            words.released,
            words.colon,
            document.released_by,
            date(document.released_at, document.language)
        ),
    );
    if document.live_in.is_empty() {
        pdf.text(Style::Muted, words.live_nowhere);
    } else {
        let environments: Vec<&str> = document
            .live_in
            .iter()
            .map(|environment| match environment {
                Environment::Staging => words.staging,
                Environment::Production => words.production,
            })
            .collect();
        pdf.text(
            Style::Muted,
            &format!(
                "{}{}{}",
                words.live_in,
                words.colon,
                environments.join(", ")
            ),
        );
    }
    if !document.notes.trim().is_empty() {
        pdf.gap(4.0);
        pdf.text(Style::Body, document.notes.trim());
    }

    approvals(&mut pdf, document, words);

    if document.decisions.is_empty() {
        pdf.gap(12.0);
        pdf.text(Style::Body, words.no_decisions);
    }
    for decision in &document.decisions {
        pdf.gap(16.0);
        pdf.text(
            Style::Heading,
            &format!(
                "{} {} — {} {}",
                words.decision, decision.key, words.version, decision.version
            ),
        );
        contract(&mut pdf, decision, words);
        content(&mut pdf, decision, words);
        history(&mut pdf, decision, document.language, words);
        tests(&mut pdf, decision, words);
    }
    Ok(pdf.finish()?)
}

fn approvals(pdf: &mut Pdf, document: &PolicyDocument, words: &Words) {
    pdf.gap(12.0);
    pdf.text(Style::Label, words.approvals);
    if document.approvals.is_empty() {
        pdf.text(Style::Body, words.no_approvals);
        return;
    }
    for approval in &document.approvals {
        let status = match approval.status {
            ApprovalStatus::Pending => words.pending,
            ApprovalStatus::Approved => words.approved,
            ApprovalStatus::Rejected => words.rejected,
            ApprovalStatus::Withdrawn => words.withdrawn,
        };
        pdf.text(
            Style::Body,
            &format!(
                "{status} — {} {} ({})",
                words.requested,
                approval.requested_by,
                date(approval.requested_at, document.language)
            ),
        );
        if let (Some(by), Some(at)) = (&approval.decided_by, approval.decided_at) {
            pdf.text(
                Style::Muted,
                &format!("{} {by} ({})", words.decided, date(at, document.language)),
            );
        }
        // A release that reads a protected characteristic says why it was let out (DNK-71).
        if let Some(justification) = &approval.justification {
            pdf.text(
                Style::Muted,
                &format!("{}{}{justification}", words.justification, words.colon),
            );
        }
        if let Some(reason) = &approval.reason {
            pdf.text(
                Style::Muted,
                &format!("{}{}{reason}", words.reason, words.colon),
            );
        }
    }
}

fn contract(pdf: &mut Pdf, decision: &DocumentDecision, words: &Words) {
    pdf.gap(6.0);
    pdf.text(Style::Label, words.contract);
    if decision.contract.is_empty() {
        pdf.text(Style::Body, words.no_contract);
        return;
    }
    for field in &decision.contract {
        // An object that holds other fields is a heading: its own fields follow it, and
        // marking it required would say nothing about what a request must actually carry.
        if field.has_properties {
            pdf.text(Style::Body, &field.path);
            continue;
        }
        let mut marks = vec![if field.required {
            words.required
        } else {
            words.optional
        }];
        if field.protected {
            marks.push(words.protected);
        }
        if field.pii {
            marks.push(words.personal);
        }
        pdf.text(
            Style::Body,
            &format!("{}{}{}", field.path, words.colon, marks.join(", ")),
        );
    }
}

fn content(pdf: &mut Pdf, decision: &DocumentDecision, words: &Words) {
    for node in &decision.nodes {
        pdf.gap(6.0);
        match node {
            Node::Table(table) => {
                let policy = if table.hit_policy == "collect" {
                    words.hit_collect
                } else {
                    words.hit_first
                };
                pdf.text(
                    Style::Label,
                    &format!("{} — {} ({policy})", table.name, words.rules),
                );
                let header: Vec<&str> = table
                    .inputs
                    .iter()
                    .chain(table.outputs.iter())
                    .map(String::as_str)
                    .collect();
                pdf.text(Style::Body, &header.join(" | "));
                if table.rows.is_empty() {
                    pdf.text(Style::Body, words.no_rows);
                }
                for row in &table.rows {
                    pdf.text(Style::Body, &row.join(" | "));
                }
            }
            Node::Expression(expression) => {
                pdf.text(
                    Style::Label,
                    &format!("{} — {}", expression.name, words.expression),
                );
                for (field, value) in &expression.entries {
                    pdf.text(Style::Body, &format!("{field}{}{value}", words.colon));
                }
            }
            // Named, never dropped: a reader must know this part of the policy exists.
            Node::Other { name, kind } => {
                pdf.text(Style::Label, &format!("{name} ({kind})"));
                pdf.text(Style::Body, words.not_rendered);
            }
        }
    }
}

/// Every version up to the one this release froze: what changed, who wrote it and when.
fn history(pdf: &mut Pdf, decision: &DocumentDecision, language: Language, words: &Words) {
    if decision.history.is_empty() {
        return;
    }
    pdf.gap(6.0);
    pdf.text(Style::Label, words.history);
    for version in &decision.history {
        let mut line = format!(
            "{} {} — {} ({})",
            words.version,
            version.number,
            version.by,
            date(version.at, language)
        );
        if let Some(restored) = version.restored_from {
            line.push_str(&format!(" — {} {restored}", words.restored));
        }
        pdf.text(Style::Body, &line);
        if !version.message.trim().is_empty() {
            pdf.text(Style::Muted, version.message.trim());
        }
    }
}

fn tests(pdf: &mut Pdf, decision: &DocumentDecision, words: &Words) {
    let TestSummary {
        passed,
        failed,
        errors,
    } = decision.tests;
    pdf.gap(6.0);
    pdf.text(Style::Label, words.tests);
    if passed + failed + errors == 0 {
        pdf.text(Style::Body, words.no_tests);
        return;
    }
    // Counted, not judged: a reader sees what ran and what it did.
    pdf.text(
        Style::Body,
        &format!(
            "{}{}{passed} · {}{}{failed} · {}{}{errors}",
            words.passed, words.colon, words.failed, words.colon, words.errors, words.colon
        ),
    );
}

#[cfg(test)]
mod tests;
