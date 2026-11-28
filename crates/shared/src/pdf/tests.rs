#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use chrono::TimeZone;

#[test]
fn long_text_wraps_inside_the_page_and_long_words_break() {
    let width = PAGE.0 - 2.0 * MARGIN;
    let long = "word ".repeat(200);
    let lines = wrap(REGULAR, BODY, &long, width);
    assert!(lines.len() > 5);
    let unbroken = "x".repeat(400);
    assert!(wrap(REGULAR, BODY, &unbroken, width).len() > 1);
}

#[test]
fn dates_are_written_in_the_documents_language() {
    let at = Utc.with_ymd_and_hms(2026, 8, 3, 9, 5, 0).unwrap();
    assert_eq!(date(at, Language::En), "3 August 2026, 09:05 UTC");
    assert_eq!(date(at, Language::Fr), "3 août 2026, 09:05 UTC");
}

#[test]
fn a_document_longer_than_a_page_continues_on_the_next_one() {
    let mut pdf = Pdf::new("Page").unwrap();
    pdf.text(Style::Title, "A long document");
    for line in 0..200 {
        pdf.text(Style::Body, &format!("Line {line}"));
    }
    assert!(pdf.pages.len() > 1, "200 lines do not fit on one page");
    let bytes = pdf.finish().unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn an_empty_document_is_still_a_page() {
    let pdf = Pdf::new("Page").unwrap();
    assert_eq!(pdf.pages.len(), 1);
    assert!(pdf.finish().unwrap().starts_with(b"%PDF-"));
}

/// A gap wide enough to fill the page moves the next line to a new one, so a caller cannot
/// push text off the bottom by asking for space.
#[test]
fn a_gap_past_the_bottom_of_the_page_starts_the_next_one() {
    let mut pdf = Pdf::new("Page").unwrap();
    pdf.text(Style::Body, "First");
    pdf.gap(PAGE.1);
    pdf.text(Style::Body, "Second");
    assert_eq!(pdf.pages.len(), 2);
}
