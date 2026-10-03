//! The release artifact Donka Runtime reads (docs/artifact-format.md): one
//! zip per project and environment, `.config/project.json` (format 2) and one
//! entry per decision, named by its key so graphs find each other. A release
//! pulled by CI outside an environment (DNK-20) has no environment and lists
//! no token, so a Runtime given it refuses every request.
//!
//! The zip is deterministic: the same input gives the same bytes, so a
//! checksum promised before a download matches the download.

use serde_json::{json, Value};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// The `.config/project.json` format this Studio writes.
pub const FORMAT_VERSION: &str = "2";

/// Everything an artifact holds.
pub struct ArtifactInput<'a> {
    pub project_id: String,
    pub project_key: &'a str,
    pub project_name: &'a str,
    pub release_id: String,
    pub release_version: String,
    /// The environment deployed to; `None` for a release pulled on its own.
    pub deployment: Option<DeploymentInput<'a>>,
    /// `(decision key, JDM graph)`.
    pub decisions: Vec<(String, Value)>,
}

/// A deployment of the release to an environment.
pub struct DeploymentInput<'a> {
    pub environment: &'a str,
    pub id: String,
    pub deployed_at: String,
    /// `(token id, sha256 hex)` of the environment's live tokens.
    pub token_hashes: Vec<(String, String)>,
}

/// The object an artifact is written to in the bucket: `staging/credit-pme`.
pub fn object_key(environment: &str, project_key: &str) -> String {
    format!("{environment}/{project_key}")
}

pub fn config(input: &ArtifactInput<'_>) -> Value {
    let mut config = json!({
        "version": FORMAT_VERSION,
        "project": { "id": input.project_id, "key": input.project_key, "name": input.project_name },
        "release": {
            "id": input.release_id,
            "version": input.release_version,
            "status": "published"
        },
        "accessTokenHashes": [],
    });
    if let Some(deployment) = &input.deployment {
        config["environment"] = json!({
            // Environments are a fixed pair per project; this id is stable for both.
            "id": format!("{}/{}", input.project_id, deployment.environment),
            "key": deployment.environment,
            "name": environment_name(deployment.environment)
        });
        config["accessTokenHashes"] = deployment
            .token_hashes
            .iter()
            .map(|(id, hash)| {
                json!({
                    "id": id,
                    "environment": deployment.environment,
                    "algorithm": "sha256",
                    "hash": hash
                })
            })
            .collect();
        config["deployment"] = json!({ "id": deployment.id, "createdAt": deployment.deployed_at });
    }
    config
}

/// Zips the artifact.
pub fn build(input: &ArtifactInput<'_>) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let mut add = |name: &str, value: &Value| -> Result<(), String> {
        zip.start_file(name, options)
            .map_err(|err| err.to_string())?;
        let body = serde_json::to_vec(value).map_err(|err| err.to_string())?;
        zip.write_all(&body).map_err(|err| err.to_string())
    };
    add(".config/project.json", &config(input))?;
    for (key, content) in &input.decisions {
        add(key, content)?;
    }
    let cursor = zip.finish().map_err(|err| err.to_string())?;
    Ok(cursor.into_inner())
}

fn environment_name(environment: &str) -> &'static str {
    match environment {
        "production" => "Production",
        _ => "Staging",
    }
}

#[cfg(test)]
mod tests {
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
        let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
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
}
