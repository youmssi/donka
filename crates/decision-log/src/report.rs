//! The access report a person receives (DNK-75): every decision found about them, with what it
//! read, what it answered and why, as a PDF in English or French.
//!
//! The layout is plain text on A4 pages: the font's own widths decide where lines wrap, so the
//! text stays text (selectable, searchable) and nothing depends on a browser.

use chrono::{DateTime, Utc};
use donka_identity::Locale;
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use krilla::Document;
use serde_json::Value;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// Geist, by Vercel and basement.studio, under the SIL Open Font License (fonts/OFL.txt).
const REGULAR: &[u8] = include_bytes!("../fonts/Geist-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../fonts/Geist-SemiBold.ttf");

const PAGE: (f32, f32) = (595.0, 842.0);
const MARGIN: f32 = 56.0;
const BODY: f32 = 10.0;

/// What the report says, gathered by the caller.
#[derive(Debug, Clone)]
pub struct AccessReport {
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

/// Why a report could not be produced: the embedded fonts or the PDF writer failed.
#[derive(Debug, thiserror::Error)]
#[error("the access report could not be written: {0}")]
pub struct ReportError(String);

/// The report as a PDF document.
pub fn render(report: &AccessReport) -> Result<Vec<u8>, ReportError> {
    let words = match report.locale {
        Locale::En => &EN,
        Locale::Fr => &FR,
    };
    let mut pdf = Pdf::new(words.page)?;
    pdf.text(Style::Title, words.title);
    pdf.text(
        Style::Muted,
        &format!(
            "{} — {} {}",
            report.project,
            words.generated,
            date(report.generated_at, report.locale)
        ),
    );
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
                date(record.evaluated_at, report.locale)
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
                    date(contest.opened_at, report.locale)
                ),
            );
            pdf.text(Style::Body, &contest.reason);
            if let Some(review) = &contest.review {
                pdf.text(Style::Body, review);
            }
        }
    }
    pdf.finish()
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

fn date(at: DateTime<Utc>, locale: Locale) -> String {
    match locale {
        Locale::En => at.format("%-d %B %Y, %H:%M UTC").to_string(),
        Locale::Fr => {
            const MONTHS: [&str; 12] = [
                "janvier",
                "février",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "août",
                "septembre",
                "octobre",
                "novembre",
                "décembre",
            ];
            let month = MONTHS[at.format("%m").to_string().parse::<usize>().unwrap_or(1) - 1];
            format!(
                "{} {month} {}, {} UTC",
                at.format("%-d"),
                at.format("%Y"),
                at.format("%H:%M")
            )
        }
    }
}

#[derive(Clone, Copy)]
enum Style {
    Title,
    Heading,
    Label,
    Body,
    Muted,
}

impl Style {
    fn size(self) -> f32 {
        match self {
            Style::Title => 20.0,
            Style::Heading => 13.0,
            Style::Label => BODY,
            Style::Body | Style::Muted => BODY,
        }
    }

    fn bold(self) -> bool {
        matches!(self, Style::Title | Style::Heading | Style::Label)
    }
}

/// Lines laid out top to bottom, a new page when one is full.
struct Pdf {
    document: Document,
    pages: Vec<Vec<(Point, Style, String)>>,
    y: f32,
    regular: Font,
    semibold: Font,
    page_word: &'static str,
}

impl Pdf {
    fn new(page_word: &'static str) -> Result<Self, ReportError> {
        let font = |bytes: &'static [u8]| {
            Font::new(bytes.into(), 0)
                .ok_or_else(|| ReportError("an embedded font cannot be read".to_owned()))
        };
        Ok(Self {
            document: Document::new(),
            pages: vec![Vec::new()],
            y: MARGIN,
            regular: font(REGULAR)?,
            semibold: font(SEMIBOLD)?,
            page_word,
        })
    }

    fn gap(&mut self, points: f32) {
        self.y += points;
    }

    fn text(&mut self, style: Style, text: &str) {
        let size = style.size();
        let leading = size * 1.45;
        let bytes = if style.bold() { SEMIBOLD } else { REGULAR };
        for line in wrap(bytes, size, text, PAGE.0 - 2.0 * MARGIN) {
            if self.y + leading > PAGE.1 - MARGIN {
                self.pages.push(Vec::new());
                self.y = MARGIN;
            }
            self.y += leading;
            if let Some(page) = self.pages.last_mut() {
                page.push((Point::from_xy(MARGIN, self.y), style, line));
            }
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, ReportError> {
        let total = self.pages.len();
        for (number, lines) in std::mem::take(&mut self.pages).into_iter().enumerate() {
            let Some(settings) = PageSettings::from_wh(PAGE.0, PAGE.1) else {
                continue;
            };
            let mut page = self.document.start_page_with(settings);
            let mut surface = page.surface();
            for (at, style, line) in lines {
                let font = if style.bold() {
                    self.semibold.clone()
                } else {
                    self.regular.clone()
                };
                surface.draw_text(at, font, style.size(), &line, false, TextDirection::Auto);
            }
            let footer = format!("{} {} / {total}", self.page_word, number + 1);
            surface.draw_text(
                Point::from_xy(MARGIN, PAGE.1 - MARGIN / 2.0),
                self.regular.clone(),
                8.0,
                &footer,
                false,
                TextDirection::Auto,
            );
            surface.finish();
            page.finish();
        }
        self.document
            .finish()
            .map_err(|err| ReportError(format!("{err:?}")))
    }
}

/// `text` in lines no wider than `width`, breaking between words (a word longer than a line
/// is broken where it overflows).
fn wrap(font: &[u8], size: f32, text: &str, width: f32) -> Vec<String> {
    let Ok(face) = FontRef::new(font) else {
        return vec![text.to_owned()];
    };
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(Size::new(size), LocationRef::default());
    let measure = |text: &str| -> f32 {
        text.chars()
            .map(|c| {
                charmap
                    .map(c)
                    .and_then(|glyph| metrics.advance_width(glyph))
                    .unwrap_or(size * 0.5)
            })
            .sum()
    };
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure(&candidate) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A word wider than the line goes on as many lines as it needs.
            let mut piece = String::new();
            for c in word.chars() {
                piece.push(c);
                if measure(&piece) > width {
                    piece.pop();
                    lines.push(std::mem::take(&mut piece));
                    piece.push(c);
                }
            }
            line = piece;
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests;
