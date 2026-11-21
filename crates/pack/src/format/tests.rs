use super::*;
use serde_json::json;

fn starter(key: &str) -> Pack {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs")
        .join(key);
    Pack::from_dir(&dir).unwrap()
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

#[test]
fn a_pack_survives_a_round_trip_through_its_file() {
    let pack = starter("retail-credit");
    assert_eq!(
        pack.manifest.decisions,
        ["affordability", "scorecard", "credit-application"]
    );
    assert!(pack.guides.en.is_some() && pack.guides.fr.is_some());
    let again = Pack::from_zip(&pack.to_zip().unwrap()).unwrap();
    assert_eq!(again, pack);
}

#[test]
fn a_zip_of_the_pack_folder_reads_like_the_folder() {
    let pack = starter("sme-treasury");
    let flat = pack.to_zip().unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(flat)).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.push((format!("sme-treasury/{}", entry.name().unwrap()), bytes));
    }
    let nested: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    assert_eq!(Pack::from_zip(&zip_of(&nested)).unwrap(), pack);
}

#[test]
fn a_broken_pack_file_says_why() {
    let manifest = |decisions: Value| {
        serde_json::to_vec(&json!({
            "format": 1, "key": "p", "name": { "en": "P", "fr": "P" },
            "description": { "en": "", "fr": "" }, "decisions": decisions
        }))
        .unwrap()
    };
    let graph = serde_json::to_vec(&json!({ "nodes": [], "edges": [] })).unwrap();
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (b"not a zip".to_vec(), "not a zip file"),
        (
            zip_of(&[("decisions/a.json", &graph)]),
            "pack.json is missing",
        ),
        (
            zip_of(&[("pack.json", &manifest(json!(["a"])))]),
            "decisions/a.json is missing",
        ),
        (
            zip_of(&[("pack.json", &manifest(json!([])))]),
            "at least one decision",
        ),
        (
            zip_of(&[
                ("pack.json", &manifest(json!(["a"]))),
                ("decisions/a.json", &graph),
                ("../evil.json", b"{}"),
            ]),
            "unsafe path",
        ),
    ];
    for (bytes, reason) in cases {
        let err = Pack::from_zip(&bytes).unwrap_err().to_string();
        assert!(err.contains(reason), "{reason}: {err}");
    }
    assert!(matches!(
        Pack::from_zip(&vec![0u8; MAX_FILE_BYTES + 1]),
        Err(PackError::TooLarge)
    ));
}

#[test]
fn decisions_come_after_the_ones_they_call() {
    let pack = starter("retail-credit");
    assert_eq!(
        dependency_order(&pack.decisions).unwrap(),
        ["affordability", "scorecard", "credit-application"]
    );
    let mut reversed = pack.clone();
    reversed.manifest.decisions.reverse();
    assert!(reversed
        .check()
        .unwrap_err()
        .to_string()
        .contains("calls scorecard"));

    let calling = |key: &str| {
        json!({ "nodes": [{ "id": "d", "name": "d", "type": "decisionNode",
                "position": { "x": 0, "y": 0 }, "content": { "key": key } }], "edges": [] })
    };
    let looped = BTreeMap::from([
        ("a".to_owned(), calling("b")),
        ("b".to_owned(), calling("a")),
    ]);
    assert!(dependency_order(&looped).is_err());
}

#[test]
fn two_scenarios_of_a_decision_need_different_names() {
    let mut pack = starter("retail-credit");
    let mut twin = pack.scenarios[0].clone();
    twin.name = twin.name.to_uppercase();
    pack.scenarios.push(twin);
    assert!(pack
        .check()
        .unwrap_err()
        .to_string()
        .contains("two scenarios named"));
}
