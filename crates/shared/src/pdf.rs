//! Plain-text documents as PDF: A4 pages, lines laid out top to bottom, a new page when one
//! is full. The font's own widths decide where lines wrap, so the text stays text (selectable,
//! searchable) and nothing depends on a browser being installed.
//!
//! This is layout only. What a document says, and in which words, belongs to the module that
//! owns the data: the access report a person receives (DNK-75) and the policy document an
//! auditor reads (DNK-79) both build their pages here.

use crate::language::Language;
use chrono::{DateTime, Utc};
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use krilla::Document;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// Geist, by Vercel and basement.studio, under the SIL Open Font License (fonts/OFL.txt).
const REGULAR: &[u8] = include_bytes!("../fonts/Geist-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../fonts/Geist-SemiBold.ttf");

const PAGE: (f32, f32) = (595.0, 842.0);
const MARGIN: f32 = 56.0;
const BODY: f32 = 10.0;

/// Why a document could not be written: the embedded fonts or the PDF writer failed.
#[derive(Debug, thiserror::Error)]
#[error("the document could not be written: {0}")]
pub struct PdfError(String);

/// How a line is set. Sizes and weights are the document's, not the caller's, so two
/// documents from different modules read as one family.
#[derive(Debug, Clone, Copy)]
pub enum Style {
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
            Style::Label | Style::Body | Style::Muted => BODY,
        }
    }

    fn bold(self) -> bool {
        matches!(self, Style::Title | Style::Heading | Style::Label)
    }
}

/// Lines laid out top to bottom, a new page when one is full.
pub struct Pdf {
    document: Document,
    pages: Vec<Vec<(Point, Style, String)>>,
    y: f32,
    regular: Font,
    semibold: Font,
    page_word: String,
}

impl Pdf {
    /// A new document whose page footers read `<page_word> 1 / 3`, in the caller's language.
    pub fn new(page_word: &str) -> Result<Self, PdfError> {
        let font = |bytes: &'static [u8]| {
            Font::new(bytes.into(), 0)
                .ok_or_else(|| PdfError("an embedded font cannot be read".to_owned()))
        };
        Ok(Self {
            document: Document::new(),
            pages: vec![Vec::new()],
            y: MARGIN,
            regular: font(REGULAR)?,
            semibold: font(SEMIBOLD)?,
            page_word: page_word.to_owned(),
        })
    }

    /// Vertical space before the next line.
    pub fn gap(&mut self, points: f32) {
        self.y += points;
    }

    /// One piece of text, wrapped to the page and broken across pages as it needs.
    pub fn text(&mut self, style: Style, text: &str) {
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

    /// The document's bytes, each page numbered.
    pub fn finish(mut self) -> Result<Vec<u8>, PdfError> {
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
            .map_err(|err| PdfError(format!("{err:?}")))
    }
}

/// An instant as a document writes it, in the document's language and always in UTC, because
/// a reader comparing two documents must not have to guess which clock each was written by.
pub fn date(at: DateTime<Utc>, language: Language) -> String {
    match language {
        Language::En => at.format("%-d %B %Y, %H:%M UTC").to_string(),
        Language::Fr => {
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
