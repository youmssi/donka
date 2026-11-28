#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use crate::{ReleaseSummary, ReleasedDecision, SemVer};
use chrono::TimeZone;
use donka_engine::outline::{Expression, Table};
use serde_json::json;
use uuid::Uuid;

fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, 9, 30, 0).unwrap()
}

fn field(path: &str, required: bool, protected: bool) -> Field {
    Field {
        path: path.to_owned(),
        required,
        pii: false,
        identifier: false,
        protected,
        schema: json!({ "type": "number" }),
        has_properties: false,
    }
}

/// An object whose fields follow it: a heading, not a value a request carries.
fn parent(path: &str) -> Field {
    Field {
        has_properties: true,
        schema: json!({ "type": "object" }),
        ..field(path, true, false)
    }
}

fn decision() -> DocumentDecision {
    DocumentDecision {
        key: "limit".to_owned(),
        version: 3,
        tests: TestSummary {
            passed: 4,
            failed: 1,
            errors: 0,
        },
        history: vec![DocumentVersion {
            number: 3,
            message: "Raised the ceiling".to_owned(),
            by: "grace@bank.example".to_owned(),
            at: at(18),
            restored_from: Some(1),
        }],
        contract: vec![
            parent("applicant"),
            field("applicant.income", true, false),
            field("applicant.region", false, true),
        ],
        nodes: vec![
            Node::Table(Table {
                name: "Limit".to_owned(),
                hit_policy: "first".to_owned(),
                inputs: vec!["Income".to_owned()],
                outputs: vec!["limit".to_owned()],
                rows: vec![vec!["> 100000".to_owned(), "500000".to_owned()]],
            }),
            Node::Expression(Expression {
                name: "Ratio".to_owned(),
                entries: vec![("dti".to_owned(), "repayment / income".to_owned())],
            }),
            Node::Other {
                name: "Bureau".to_owned(),
                kind: "donka.connector".to_owned(),
            },
        ],
    }
}

fn document(language: Language) -> PolicyDocument {
    PolicyDocument {
        organisation: Some("Microfinance SA".to_owned()),
        project: "Crédit PME".to_owned(),
        version: "1.4.0".to_owned(),
        notes: "Ceiling raised for salaried applicants.".to_owned(),
        released_by: "grace@bank.example".to_owned(),
        released_at: at(20),
        generated_at: at(28),
        language,
        live_in: vec![Environment::Production],
        decisions: vec![decision()],
        approvals: vec![DocumentApproval {
            status: ApprovalStatus::Approved,
            requested_by: "grace@bank.example".to_owned(),
            requested_at: at(19),
            decided_by: Some("alan@bank.example".to_owned()),
            decided_at: Some(at(20)),
            justification: Some("Region is used for branch routing only.".to_owned()),
            reason: None,
        }],
    }
}

#[test]
fn a_release_is_a_pdf_in_either_language() {
    for language in [Language::En, Language::Fr] {
        let pdf = render(&document(language)).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{language:?}");
    }
}

#[test]
fn the_subtitle_names_the_holder_the_release_and_when_it_was_written() {
    let mut document = document(Language::En);
    let named = subtitle(&document, &EN);
    assert!(
        named.starts_with("Microfinance SA · Crédit PME 1.4.0 — written on "),
        "{named}"
    );
    document.organisation = None;
    assert!(
        subtitle(&document, &EN).starts_with("Crédit PME 1.4.0 — "),
        "an installation that names no organisation still names the project"
    );
}

/// A document that renders every node kind it can and names the ones it cannot is the whole
/// point: nothing in the policy may be invisible to the reader.
#[test]
fn a_release_with_nothing_in_it_still_says_so() {
    let mut empty = document(Language::En);
    empty.decisions = Vec::new();
    empty.approvals = Vec::new();
    empty.live_in = Vec::new();
    empty.notes = "   ".to_owned();
    assert!(render(&empty).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn a_decision_without_a_contract_or_rows_renders_rather_than_failing() {
    let mut bare = document(Language::Fr);
    bare.decisions[0].contract = Vec::new();
    bare.decisions[0].history = Vec::new();
    bare.decisions[0].tests = TestSummary::default();
    bare.decisions[0].nodes = vec![Node::Table(Table {
        name: "Vide".to_owned(),
        hit_policy: "collect".to_owned(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        rows: Vec::new(),
    })];
    assert!(render(&bare).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn an_approval_still_waiting_has_no_one_who_decided_it() {
    let mut waiting = document(Language::En);
    waiting.approvals[0] = DocumentApproval {
        status: ApprovalStatus::Pending,
        requested_by: "grace@bank.example".to_owned(),
        requested_at: at(27),
        decided_by: None,
        decided_at: None,
        justification: None,
        reason: None,
    };
    assert!(render(&waiting).unwrap().starts_with(b"%PDF-"));
}

/// Both languages name every word the document uses, or a reader gets an English page in a
/// French document without anyone noticing.
#[test]
fn the_two_languages_say_the_same_things() {
    let en = render(&document(Language::En)).unwrap();
    let fr = render(&document(Language::Fr)).unwrap();
    assert_ne!(en, fr, "the two documents are not the same bytes");
    assert_ne!(EN.title, FR.title);
    assert_ne!(EN.no_decisions, FR.no_decisions);
    assert_ne!(EN.not_rendered, FR.not_rendered);
    assert_ne!(EN.hit_collect, FR.hit_collect);
}

/// The document is assembled from what the modules already expose, so a reader sees the
/// policy as it was frozen — not as the drafts have moved on since.
#[test]
fn a_document_holds_the_release_its_versions_and_only_its_own_approvals() {
    let release_id = Uuid::new_v4();
    let grace = Uuid::new_v4();
    let alan = Uuid::new_v4();
    let release = Release {
        summary: ReleaseSummary {
            id: release_id,
            version: SemVer {
                major: 1,
                minor: 4,
                patch: 0,
            },
            notes: "Ceiling raised".to_owned(),
            created_by: grace,
            created_at: at(20),
            decisions: 1,
            tests: TestSummary::default(),
            live_in: vec![Environment::Production],
        },
        decisions: vec![ReleasedDecision {
            decision_id: Uuid::new_v4(),
            key: "limit".to_owned(),
            version: 2,
            tests: TestSummary {
                passed: 3,
                failed: 0,
                errors: 0,
            },
        }],
    };
    let contents = BTreeMap::from([(
        "limit".to_owned(),
        json!({
            "nodes": [
                { "id": "i", "type": "inputNode", "name": "Request",
                  "content": { "schema": "{\"type\":\"object\",\"properties\":{\"income\":{\"type\":\"number\"}},\"required\":[\"income\"]}" } },
                { "id": "t", "type": "decisionTableNode", "name": "Limit",
                  "content": { "hitPolicy": "first",
                               "inputs": [{ "id": "c1", "field": "income" }],
                               "outputs": [{ "id": "c2", "field": "limit" }],
                               "rules": [{ "_id": "r", "c1": "> 10", "c2": "5" }] } }
            ],
            "edges": []
        }),
    )]);
    let version = |number: i32| VersionSummary {
        number,
        message: format!("Version {number}"),
        created_at: at(10 + number as u32),
        created_by: grace,
        restored_from: None,
        tests: TestSummary::default(),
    };
    let histories =
        BTreeMap::from([("limit".to_owned(), vec![version(3), version(2), version(1)])]);
    let approval = |release: Uuid| Approval {
        id: Uuid::new_v4(),
        release_id: release,
        release_version: SemVer::FIRST,
        release_notes: String::new(),
        release_created_by: grace,
        requested_by: grace,
        requested_at: at(19),
        justification: None,
        status: ApprovalStatus::Approved,
        decided_by: Some(alan),
        decided_at: Some(at(20)),
        reason: None,
        deployment_id: None,
    };
    let approvals = [approval(release_id), approval(Uuid::new_v4())];
    let who = |id: Uuid| {
        if id == grace {
            "grace@bank.example".to_owned()
        } else {
            "alan@bank.example".to_owned()
        }
    };

    let document = assemble(
        About {
            organisation: Some("Microfinance SA".to_owned()),
            project: "Crédit PME".to_owned(),
            language: Language::En,
            generated_at: at(28),
        },
        &release,
        &contents,
        &histories,
        &approvals,
        &who,
    );

    assert_eq!(document.version, "1.4.0");
    assert_eq!(document.released_by, "grace@bank.example");
    assert_eq!(document.live_in, vec![Environment::Production]);
    // Another release's approval belongs in that release's document, not this one.
    assert_eq!(document.approvals.len(), 1);
    assert_eq!(
        document.approvals[0].decided_by.as_deref(),
        Some("alan@bank.example")
    );

    let decision = &document.decisions[0];
    // The history stops at the frozen version: version 3 came after this release.
    assert_eq!(
        decision
            .history
            .iter()
            .map(|v| v.number)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(decision.contract.len(), 1, "{:?}", decision.contract);
    assert_eq!(decision.contract[0].path, "income");
    assert!(decision.contract[0].required);
    assert_eq!(decision.nodes.len(), 1, "the input node carries no policy");
    assert!(render(&document).unwrap().starts_with(b"%PDF-"));
}

/// A decision whose content never reached the release renders as itself, with nothing made up.
#[test]
fn a_decision_without_content_or_history_is_still_listed() {
    let release = Release {
        summary: ReleaseSummary {
            id: Uuid::new_v4(),
            version: SemVer::FIRST,
            notes: String::new(),
            created_by: Uuid::new_v4(),
            created_at: at(20),
            decisions: 1,
            tests: TestSummary::default(),
            live_in: Vec::new(),
        },
        decisions: vec![ReleasedDecision {
            decision_id: Uuid::new_v4(),
            key: "pricing".to_owned(),
            version: 1,
            tests: TestSummary::default(),
        }],
    };
    let document = assemble(
        About {
            organisation: None,
            project: "Retail".to_owned(),
            language: Language::Fr,
            generated_at: at(28),
        },
        &release,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &|_| "nobody@bank.example".to_owned(),
    );
    let decision = &document.decisions[0];
    assert_eq!(decision.key, "pricing");
    assert!(decision.contract.is_empty());
    assert!(decision.nodes.is_empty());
    assert!(decision.history.is_empty());
    assert!(render(&document).unwrap().starts_with(b"%PDF-"));
}
