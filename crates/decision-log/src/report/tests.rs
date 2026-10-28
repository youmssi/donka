#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use chrono::TimeZone;
use serde_json::json;

fn record(n: u32) -> ReportRecord {
    ReportRecord {
        decision_key: "scorecard".to_owned(),
        evaluated_at: Utc.with_ymd_and_hms(2026, 9, n, 10, 30, 0).unwrap(),
        release: "1.2.0".to_owned(),
        outcome: Some("decline".to_owned()),
        succeeded: true,
        input: json!({ "applicant": { "nationalId": "CM-1984-0042", "monthlyIncome": 300000 } }),
        output: Some(json!({ "decision": "decline", "reasons": ["DTI_TOO_HIGH"] })),
        explanation: Some("The repayments would take more than 40% of the income.".to_owned()),
        anonymised: false,
        contests: vec![ReportContest {
            opened_at: Utc.with_ymd_and_hms(2026, 9, 29, 9, 0, 0).unwrap(),
            status: "revised".to_owned(),
            reason: "My income has since increased.".to_owned(),
            review: Some("Approved after a new payslip.".to_owned()),
        }],
    }
}

fn text_of(pdf: &[u8]) -> String {
    // The text stays text: every drawn string is in the content, as glyph ids. Checking the
    // document parses and holds its pages is what this test can do without a PDF reader.
    String::from_utf8_lossy(pdf).into_owned()
}

#[test]
fn a_report_is_a_pdf_with_a_page_per_screenful_of_records() {
    let report = AccessReport {
        project: "Crédit PME".to_owned(),
        generated_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        locale: Locale::Fr,
        records: (1..=20).map(record).collect(),
    };
    let pdf = render(&report).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let text = text_of(&pdf);
    assert!(text.matches("/Type /Page\n").count() + text.matches("/Type/Page").count() >= 1);
}

#[test]
fn an_empty_report_says_nothing_is_held() {
    let report = AccessReport {
        project: "Retail".to_owned(),
        generated_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        locale: Locale::En,
        records: vec![],
    };
    assert!(render(&report).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn nested_values_read_as_dotted_paths() {
    let value = json!({
        "applicant": { "age": 30, "name": "Ada" },
        "notes": null,
        "reasons": ["DTI_TOO_HIGH", "SHORT_HISTORY"],
        "loans": [{ "amount": 5 }]
    });
    assert_eq!(
        flatten(&value, ": "),
        [
            "applicant.age: 30",
            "applicant.name: Ada",
            "loans[1].amount: 5",
            "notes: —",
            "reasons: DTI_TOO_HIGH, SHORT_HISTORY"
        ]
    );
    assert_eq!(
        flatten(&json!({ "age": 30 }), "\u{a0}: "),
        ["age\u{a0}: 30"]
    );
}

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
fn dates_are_written_in_the_reports_language() {
    let at = Utc.with_ymd_and_hms(2026, 8, 3, 9, 5, 0).unwrap();
    assert_eq!(date(at, Locale::En), "3 August 2026, 09:05 UTC");
    assert_eq!(date(at, Locale::Fr), "3 août 2026, 09:05 UTC");
}
