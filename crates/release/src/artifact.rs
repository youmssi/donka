//! The release artifact Donka Runtime reads (docs/artifact-format.md): one
//! zip per project and environment, `.config/project.json` (format 2) and one
//! entry per decision, named by its key so graphs find each other. A release
//! pulled by CI outside an environment (DNK-20) has no environment and lists
//! no token, so a Runtime given it refuses every request.
//!
//! The zip is deterministic: the same input gives the same bytes, so a
//! checksum promised before a download matches the download.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
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
    write_zip(&entries_of(input, None)?)
}

/// The installation's artifact signing key (DNK-87): its id names it to the
/// Runtime, its seed signs. `Debug` shows the id only: the seed stays secret.
#[derive(Clone, PartialEq, Eq)]
pub struct Signing {
    pub key_id: String,
    pub seed: [u8; 32],
}

impl std::fmt::Debug for Signing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signing({})", self.key_id)
    }
}

/// Zips the artifact and signs it (DNK-87): `.config/signature.json` names the
/// key, the digest of the artifact's canonical content, and the Ed25519
/// signature over that digest. A Runtime holding the installation's public
/// key refuses anything else; without one it serves as before.
pub fn build_signed(input: &ArtifactInput<'_>, signing: &Signing) -> Result<Vec<u8>, String> {
    let digest = digest(&entries_of(input, None)?);
    let key = ed25519_dalek::SigningKey::from_bytes(&signing.seed);
    let signature = ed25519_dalek::Signer::sign(&key, &digest_bytes(&digest)?);
    let file = json!({
        "format": "donka-artifact/1",
        "keyId": signing.key_id,
        "digest": digest,
        "signature": B64.encode(signature.to_bytes()),
    });
    write_zip(&entries_of(input, Some(file))?)
}

/// The signature entry inside the artifact zip (DNK-87).
pub const SIGNATURE_ENTRY: &str = ".config/signature.json";

/// SHA-256 (hex) of the canonical content: every entry except the signature,
/// sorted by name, name and content each followed by one zero byte. The
/// Runtime computes the same digest from the artifact it reads.
pub fn digest(entries: &[(String, Vec<u8>)]) -> String {
    use sha2::Digest as _;
    let mut sorted: Vec<&(String, Vec<u8>)> = entries
        .iter()
        .filter(|(name, _)| name != SIGNATURE_ENTRY)
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = sha2::Sha256::new();
    for (name, body) in sorted {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(body);
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn digest_bytes(hex: &str) -> Result<Vec<u8>, String> {
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| "the artifact digest is not hex".to_owned())
        })
        .collect()
}

fn entries_of(
    input: &ArtifactInput<'_>,
    signature: Option<Value>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut values = vec![(".config/project.json".to_owned(), config(input))];
    if let Some(file) = signature {
        values.push((SIGNATURE_ENTRY.to_owned(), file));
    }
    for (key, content) in &input.decisions {
        values.push((key.clone(), content.clone()));
    }
    // The input contract of each decision that has one (DNK-37), where Runtimes that do not
    // know it never look: everything under `.config/` is skipped when loading decisions.
    for (key, content) in &input.decisions {
        if let Ok(Some(schema)) = contract::input_schema(content) {
            values.push((contract_path(key), schema));
        }
    }
    values
        .into_iter()
        .map(|(name, value)| {
            serde_json::to_vec(&value)
                .map(|body| (name, body))
                .map_err(|err| err.to_string())
        })
        .collect()
}

fn write_zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    for (name, body) in entries {
        zip.start_file(name, options)
            .map_err(|err| err.to_string())?;
        zip.write_all(body).map_err(|err| err.to_string())?;
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
