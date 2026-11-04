use super::*;
use std::io::Read;
use zip::ZipArchive;

fn input() -> ArtifactInput<'static> {
    ArtifactInput {
        project_id: "p-1".into(),
        project_key: "credit-pme",
        project_name: "Crédit PME",
        release_id: "r-1".into(),
        release_version: "1.2.0".into(),
        deployment: Some(DeploymentInput {
            environment: "staging",
            id: "d-1".into(),
            deployed_at: "2026-10-02T09:00:00Z".into(),
            token_hashes: vec![("t-1".into(), donka_shared::secret::hash("dnk_test_token"))],
        }),
        decisions: vec![
            (
                "bureau/normalize".into(),
                json!({ "nodes": [], "edges": [] }),
            ),
            ("person-score".into(), json!({ "nodes": [], "edges": [] })),
        ],
    }
}

#[test]
fn token_hashes_match_the_published_vector() {
    assert_eq!(
        donka_shared::secret::hash("dnk_test_token"),
        "d4d813b79f07c455e68458c955824329d902dd5f8e7b0c250fb3f05b3f68c840"
    );
}

#[test]
fn the_zip_holds_the_config_and_one_entry_per_decision_key() {
    let bytes = build(&input()).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    assert_eq!(
        names,
        [".config/project.json", "bureau/normalize", "person-score"]
    );
    let mut config = String::new();
    zip.by_name(".config/project.json")
        .unwrap()
        .read_to_string(&mut config)
        .unwrap();
    let config: Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["version"], "2");
    assert_eq!(config["environment"]["key"], "staging");
    assert_eq!(config["release"]["version"], "1.2.0");
    assert_eq!(config["accessTokenHashes"][0]["environment"], "staging");
    assert_eq!(config["accessTokenHashes"][0]["algorithm"], "sha256");
    assert!(
        config.get("accessTokens").is_none(),
        "plain tokens are never written"
    );
    assert_eq!(object_key("staging", "credit-pme"), "staging/credit-pme");
}

#[test]
fn a_decision_with_an_input_contract_carries_it_under_config() {
    let schema = json!({ "type": "object", "properties": { "age": { "type": "integer" } } });
    let graph = json!({ "nodes": [
            { "id": "in", "type": "inputNode", "content": { "schema": schema.to_string() } }
        ], "edges": [] });
    let mut with_contract = input();
    with_contract.decisions[1].1 = graph;
    let bytes = build(&with_contract).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            ".config/project.json",
            "bureau/normalize",
            "person-score",
            ".config/contracts/person-score/input.schema.json"
        ]
    );
    let mut text = String::new();
    zip.by_name(&contract_path("person-score"))
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), schema);
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    assert_eq!(build(&input()).unwrap(), build(&input()).unwrap());
}

#[test]
fn a_release_on_its_own_has_no_environment_and_no_token() {
    let config = config(&ArtifactInput {
        deployment: None,
        ..input()
    });
    assert!(config.get("environment").is_none());
    assert!(config.get("deployment").is_none());
    assert_eq!(config["accessTokenHashes"], json!([]));
    assert_eq!(config["release"]["version"], "1.2.0");
}
