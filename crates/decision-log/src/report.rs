//! The access report a person receives (DNK-75): every decision found about them, with what it
//! read, what it answered and why, as a PDF in English or French.
//!
//! What the report says lives here; how a page is laid out lives in `donka_shared::pdf`,
//! which the policy document (DNK-79) uses as well.

use chrono::{DateTime, Utc};
use donka_identity::Locale;
use donka_shared::pdf::{date, Pdf, Style};
use serde_json::Value;

/// What the report says, gathered by the caller.
#[derive(Debug, Clone)]
pub struct AccessReport {
    /// The organisation the installation serves (DNK-74), when it is named.
    pub organisation: Option<String>,
    pub project: String,
    pub generated_at: DateTime<Utc>,
    pub locale: Locale,
    pub records: Vec<ReportRecord>,
}

#[derive(Debug, Clone)]
pub struct ReportRecord {
    pub decision_key: String,
    pub evaluated_at: DateTime<Utc>,
    pub release: String,
    pub outcome: Option<String>,
    pub succeeded: bool,
    /// What the decision read, as sent.
    pub input: Value,
    /// What it answered: the reasons are in here, as the rules give them.
    pub output: Option<Value>,
    /// A plain-language explanation, when explanations are configured.
    pub explanation: Option<String>,
    /// The person's fields were removed at their request.
    pub anonymised: bool,
    pub contests: Vec<ReportContest>,
}

#[derive(Debug, Clone)]
pub struct ReportContest {
    pub opened_at: DateTime<Utc>,
    /// `open`, `upheld` or `revised`.
    pub status: String,
    pub reason: String,
    pub review: Option<String>,
}

struct Words {
    title: &'static str,
    intro: &'static str,
    none: &'static str,
    generated: &'static str,
    decision: &'static str,
    outcome: &'static str,
    release: &'static str,
    failed: &'static str,
    read: &'static str,
    answered: &'static str,
    explanation: &'static str,
    anonymised: &'static str,
    contest: &'static str,
    open: &'static str,
    upheld: &'static str,
    revised: &'static str,
    page: &'static str,
    /// What follows a label: French sets a space before the colon.
    colon: &'static str,
}

const EN: Words = Words {
    title: "Decisions about you",
    intro: "Every automated decision this organisation holds about you, with the information it used, its result and why. You may ask for a person to review any of them.",
    none: "No decision about you is held.",
    generated: "Prepared on",
    decision: "Decision",
    outcome: "Result",
    release: "Rules version",
    failed: "The decision could not be completed.",
    read: "Information used",
    answered: "What the decision answered",
    explanation: "In plain words",
    anonymised: "Your personal details were removed from this decision at your request.",
    contest: "Your request for review",
    open: "under review",
    upheld: "decision maintained",
    revised: "decision revised",
    page: "Page",
    colon: ": ",
};

const FR: Words = Words {
    title: "Les décisions vous concernant",
    intro: "Toutes les décisions automatisées que cet établissement détient à votre sujet, avec les informations utilisées, le résultat et ses raisons. Vous pouvez demander qu'une personne réexamine chacune d'elles.",
    none: "Aucune décision vous concernant n'est conservée.",
    generated: "Établi le",
    decision: "Décision",
    outcome: "Résultat",
    release: "Version des règles",
    failed: "La décision n'a pas pu aboutir.",
    read: "Informations utilisées",
    answered: "Réponse de la décision",
    explanation: "En clair",
    anonymised: "Vos données personnelles ont été retirées de cette décision à votre demande.",
    contest: "Votre demande de réexamen",
    open: "en cours d'examen",
    upheld: "décision maintenue",
    revised: "décision révisée",
    page: "Page",
    colon: "\u{a0}: ",
};

/// Why a report could not be produced: the document writer failed.
#[derive(Debug, thiserror::Error)]
#[error("the access report could not be written: {0}")]
pub struct ReportError(#[from] donka_shared::pdf::PdfError);

/// Who holds the records, and when the report was made.
fn subtitle(report: &AccessReport, generated: &str) -> String {
    let holder = match &report.organisation {
        Some(organisation) => format!("{organisation} · {}", report.project),
        None => report.project.clone(),
    };
    format!(
        "{holder} — {generated} {}",
        date(report.generated_at, report.locale.into())
    )
}

/// The report as a PDF document.
pub fn render(report: &AccessReport) -> Result<Vec<u8>, ReportError> {
    let words = match report.locale {
        Locale::En => &EN,
        Locale::Fr => &FR,
    };
    let mut pdf = Pdf::new(words.page)?;
    pdf.text(Style::Title, words.title);
    pdf.text(Style::Muted, &subtitle(report, words.generated));
    pdf.gap(6.0);
    pdf.text(Style::Body, words.intro);
    if report.records.is_empty() {
        pdf.gap(10.0);
        pdf.text(Style::Body, words.none);
    }
    for record in &report.records {
        pdf.gap(16.0);
        pdf.text(
            Style::Heading,
            &format!(
                "{} {} — {}",
                words.decision,
                record.decision_key,
                date(record.evaluated_at, report.locale.into())
            ),
        );
        if record.succeeded {
            let outcome = record.outcome.as_deref().unwrap_or("—");
            pdf.text(
                Style::Body,
                &format!("{}{}{outcome}", words.outcome, words.colon),
            );
        } else {
            pdf.text(Style::Body, words.failed);
        }
        pdf.text(
            Style::Muted,
            &format!("{}{}{}", words.release, words.colon, record.release),
        );
        if record.anonymised {
            pdf.text(Style::Muted, words.anonymised);
        }
        pdf.gap(4.0);
        pdf.text(Style::Label, words.read);
        for line in flatten(&record.input, words.colon) {
            pdf.text(Style::Body, &line);
        }
        if let Some(output) = &record.output {
            pdf.gap(4.0);
            pdf.text(Style::Label, words.answered);
            for line in flatten(output, words.colon) {
                pdf.text(Style::Body, &line);
            }
        }
        if let Some(explanation) = &record.explanation {
            pdf.gap(4.0);
            pdf.text(Style::Label, words.explanation);
            pdf.text(Style::Body, explanation);
        }
        for contest in &record.contests {
            pdf.gap(4.0);
            let status = match contest.status.as_str() {
                "upheld" => words.upheld,
                "revised" => words.revised,
                _ => words.open,
            };
            pdf.text(
                Style::Label,
                &format!(
                    "{} ({}) — {status}",
                    words.contest,
                    date(contest.opened_at, report.locale.into())
                ),
            );
            pdf.text(Style::Body, &contest.reason);
            if let Some(review) = &contest.review {
                pdf.text(Style::Body, review);
            }
        }
    }
    Ok(pdf.finish()?)
}

/// A value as `path: value` lines, nested objects as dotted paths and lists as their items.
fn flatten(value: &Value, colon: &str) -> Vec<String> {
    fn walk(value: &Value, path: &str, colon: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(fields) if !fields.is_empty() => {
                for (key, child) in fields {
                    let path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    walk(child, &path, colon, out);
                }
            }
            Value::Array(items) if items.iter().all(|item| !item.is_object()) => {
                let items: Vec<String> = items.iter().map(scalar).collect();
                out.push(line(path, &items.join(", "), colon));
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, &format!("{path}[{}]", index + 1), colon, out);
                }
            }
            other => out.push(line(path, &scalar(other), colon)),
        }
    }
    fn scalar(value: &Value) -> String {
        match value {
            Value::String(text) => text.clone(),
            Value::Null => "—".to_owned(),
            other => other.to_string(),
        }
    }
    fn line(path: &str, value: &str, colon: &str) -> String {
        if path.is_empty() {
            value.to_owned()
        } else {
            format!("{path}{colon}{value}")
        }
    }
    let mut out = Vec::new();
    walk(value, "", colon, &mut out);
    out
}

#[cfg(test)]
mod tests;
