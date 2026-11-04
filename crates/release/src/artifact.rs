//! The release artifact Donka Runtime reads (docs/artifact-format.md): one
//! zip per project and environment, `.config/project.json` (format 2) and one
//! entry per decision, named by its key so graphs find each other. A release
//! pulled by CI outside an environment (DNK-20) has no environment and lists
//! no token, so a Runtime given it refuses every request.
//!
//! The zip is deterministic: the same input gives the same bytes, so a
//! checksum promised before a download matches the download.

use donka_engine::contract;
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
    // The input contract of each decision that has one (DNK-37), where Runtimes that do not
    // know it never look: everything under `.config/` is skipped when loading decisions.
    for (key, content) in &input.decisions {
        if let Ok(Some(schema)) = contract::input_schema(content) {
            add(&contract_path(key), &schema)?;
        }
    }
    let cursor = zip.finish().map_err(|err| err.to_string())?;
    Ok(cursor.into_inner())
}

/// Where a decision's input contract sits in the artifact.
pub fn contract_path(key: &str) -> String {
    format!(".config/contracts/{key}/input.schema.json")
}

fn environment_name(environment: &str) -> &'static str {
    match environment {
        "production" => "Production",
        _ => "Staging",
    }
}

#[cfg(test)]
#[path = "artifact/tests.rs"]
mod tests;
