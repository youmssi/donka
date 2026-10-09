//! The pack format (version 1, `packs/README.md`): a manifest, one JDM graph per decision,
//! test scenarios and an optional guide in English and French. On disk it is a directory;
//! as a file it is a zip with the same layout, at its root or inside one folder.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::PackError;

/// The only format this Studio reads and writes.
pub const FORMAT: u64 = 1;
/// A pack file larger than this is refused before it is opened.
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
/// What a pack may hold once uncompressed, so a small file cannot expand without bound.
const MAX_UNPACKED_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Localized {
    pub en: String,
    pub fr: String,
}

impl Localized {
    pub fn same(text: &str) -> Self {
        Self {
            en: text.to_owned(),
            fr: text.to_owned(),
        }
    }

    /// The text in `lang` (`fr`), English otherwise.
    pub fn get(&self, lang: &str) -> &str {
        if lang == "fr" && !self.fr.is_empty() {
            &self.fr
        } else {
            &self.en
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_field: Option<String>,
    #[serde(default)]
    pub redacted_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u64,
    pub key: String,
    pub name: Localized,
    pub description: Localized,
    /// The pack's own version, e.g. `1.2`; an exported project's release version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// ISO country code of the market the policy was written for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// The decision a caller asks for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Every decision, a decision another one calls before it.
    pub decisions: Vec<String>,
    #[serde(default)]
    pub decision_log: LogSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub decision: String,
    pub name: String,
    pub input: Value,
    pub expected: Value,
    #[serde(rename = "match")]
    pub match_mode: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Guides {
    pub en: Option<String>,
    pub fr: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub manifest: Manifest,
    /// Each decision's JDM graph, by key.
    pub decisions: BTreeMap<String, Value>,
    pub scenarios: Vec<Scenario>,
    pub guides: Guides,
}

/// The files of a pack, by their path inside it.
type Files = BTreeMap<String, Vec<u8>>;

impl Pack {
    /// Reads a pack directory (`packs/<key>`).
    pub fn from_dir(dir: &Path) -> Result<Self, PackError> {
        let mut files = Files::new();
        let read = |path: &Path| {
            std::fs::read(path)
                .map_err(|err| PackError::Invalid(format!("{}: {err}", path.display())))
        };
        for name in ["pack.json", "scenarios.json", "guide.md", "guide.fr.md"] {
            let path = dir.join(name);
            if path.is_file() {
                files.insert(name.to_owned(), read(&path)?);
            }
        }
        let decisions = dir.join("decisions");
        if decisions.is_dir() {
            collect(&decisions, "decisions", &mut files)?;
        }
        Self::from_files(files)
    }

    /// Reads a pack file (a zip).
    pub fn from_zip(bytes: &[u8]) -> Result<Self, PackError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(PackError::TooLarge);
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| PackError::Invalid("not a zip file".into()))?;
        if archive.len() > MAX_ENTRIES {
            return Err(PackError::TooLarge);
        }
        let mut files = Files::new();
        let mut unpacked = 0u64;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|_| PackError::Invalid("unreadable zip entry".into()))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .enclosed_name()
                .ok_or_else(|| PackError::Invalid(format!("unsafe path {}", entry.name())))?
                .to_string_lossy()
                .replace('\\', "/");
            let mut content = Vec::new();
            let limit = MAX_UNPACKED_BYTES - unpacked;
            (&mut entry)
                .take(limit + 1)
                .read_to_end(&mut content)
                .map_err(|_| PackError::Invalid(format!("unreadable {name}")))?;
            unpacked += content.len() as u64;
            if unpacked > MAX_UNPACKED_BYTES {
                return Err(PackError::TooLarge);
            }
            files.insert(name, content);
        }
        // A zip of the pack's folder holds everything under one directory.
        if !files.contains_key("pack.json") {
            let roots: BTreeSet<&str> = files
                .keys()
                .filter_map(|path| path.split_once('/').map(|(root, _)| root))
                .collect();
            if let [root] = roots.into_iter().collect::<Vec<_>>()[..] {
                let prefix = format!("{root}/");
                files = files
                    .into_iter()
                    .filter_map(|(path, content)| {
                        path.strip_prefix(&prefix)
                            .map(|rest| (rest.to_owned(), content))
                    })
                    .collect();
            }
        }
        Self::from_files(files)
    }

    fn from_files(files: Files) -> Result<Self, PackError> {
        let manifest: Manifest = json_file(&files, "pack.json")?;
        if manifest.format != FORMAT {
            return Err(PackError::Invalid(format!(
                "pack format {} is not supported (this Studio reads format {FORMAT})",
                manifest.format
            )));
        }
        let mut decisions = BTreeMap::new();
        for key in &manifest.decisions {
            let path = format!("decisions/{key}.json");
            decisions.insert(key.clone(), json_file(&files, &path)?);
        }
        let scenarios = if files.contains_key("scenarios.json") {
            json_file(&files, "scenarios.json")?
        } else {
            Vec::new()
        };
        let text = |name: &str| {
            files
                .get(name)
                .map(|bytes| {
                    String::from_utf8(bytes.clone())
                        .map_err(|_| PackError::Invalid(format!("{name} is not UTF-8 text")))
                })
                .transpose()
        };
        let pack = Self {
            guides: Guides {
                en: text("guide.md")?,
                fr: text("guide.fr.md")?,
            },
            manifest,
            decisions,
            scenarios,
        };
        pack.check()?;
        Ok(pack)
    }

    /// Writes the pack file: the same layout as a pack directory.
    pub fn to_zip(&self) -> Result<Vec<u8>, PackError> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let mut add = |name: &str, bytes: &[u8]| -> Result<(), PackError> {
                zip.start_file(name, options).map_err(zip_error)?;
                zip.write_all(bytes)
                    .map_err(|err| PackError::Invalid(err.to_string()))
            };
            add("pack.json", &pretty(&self.manifest)?)?;
            for key in &self.manifest.decisions {
                add(
                    &format!("decisions/{key}.json"),
                    &pretty(&self.decisions[key])?,
                )?;
            }
            add("scenarios.json", &pretty(&self.scenarios)?)?;
            if let Some(guide) = &self.guides.en {
                add("guide.md", guide.as_bytes())?;
            }
            if let Some(guide) = &self.guides.fr {
                add("guide.fr.md", guide.as_bytes())?;
            }
            zip.finish().map_err(zip_error)?;
        }
        Ok(out.into_inner())
    }

    /// Checks what can be checked without a database: every listed decision has a graph the
    /// engine accepts and a usable contract, decisions come after those they call, scenarios
    /// name a decision of the pack, and the decision-log settings are valid.
    pub fn check(&self) -> Result<(), PackError> {
        let listed: BTreeSet<&String> = self.manifest.decisions.iter().collect();
        if listed.len() != self.manifest.decisions.len() {
            return Err(PackError::Invalid(
                "a decision is listed twice in pack.json".into(),
            ));
        }
        if self.manifest.decisions.is_empty() {
            return Err(PackError::Invalid(
                "a pack holds at least one decision".into(),
            ));
        }
        for (key, content) in &self.decisions {
            donka_decision::check_decision(key, content)
                .map_err(|err| PackError::Invalid(format!("decision {key}: {err}")))?;
        }
        if let Some(entry) = &self.manifest.entry {
            if !listed.contains(entry) {
                return Err(PackError::Invalid(format!(
                    "the entry decision {entry} is not in the pack"
                )));
            }
        }
        let mut seen = BTreeSet::new();
        for key in &self.manifest.decisions {
            for called in calls(&self.decisions[key]) {
                if listed.contains(&called) && !seen.contains(&called) {
                    return Err(PackError::Invalid(format!(
                        "decision {key} calls {called}, which pack.json lists after it"
                    )));
                }
            }
            seen.insert(key.clone());
        }
        let mut names = BTreeSet::new();
        for scenario in &self.scenarios {
            if !names.insert((&scenario.decision, scenario.name.to_lowercase())) {
                return Err(PackError::Invalid(format!(
                    "decision {} has two scenarios named {}",
                    scenario.decision, scenario.name
                )));
            }
            if !listed.contains(&scenario.decision) {
                return Err(PackError::Invalid(format!(
                    "scenario {} names decision {}, which is not in the pack",
                    scenario.name, scenario.decision
                )));
            }
            donka_decision::check_scenario(scenario.fields()?)
                .map_err(|err| PackError::Invalid(format!("scenario {}: {err}", scenario.name)))?;
        }
        donka_decision_log::check_settings(&self.manifest.decision_log.change())
            .map_err(|err| PackError::Invalid(format!("decision-log settings: {err}")))?;
        Ok(())
    }
}

impl Scenario {
    pub fn fields(&self) -> Result<donka_decision::ScenarioFields, PackError> {
        Ok(donka_decision::ScenarioFields {
            name: self.name.clone(),
            input: self.input.clone(),
            expected: self.expected.clone(),
            match_mode: donka_decision::Match::parse(&self.match_mode).ok_or_else(|| {
                PackError::Invalid(format!("scenario {}: match is exact or partial", self.name))
            })?,
        })
    }
}

impl LogSettings {
    pub fn change(&self) -> donka_decision_log::SettingsChange {
        donka_decision_log::SettingsChange {
            outcome_field: Some(self.outcome_field.clone()),
            redacted_fields: Some(self.redacted_fields.clone()),
        }
    }

    pub fn is_default(&self) -> bool {
        self.outcome_field.is_none() && self.redacted_fields.is_empty()
    }
}

/// The keys of the decisions a graph calls (its decision nodes).
pub fn calls(graph: &Value) -> Vec<String> {
    graph["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node["type"] == "decisionNode")
        .filter_map(|node| node["content"]["key"].as_str().map(str::to_owned))
        .collect()
}

/// The keys in an order where each decision comes after the decisions it calls.
pub fn dependency_order(decisions: &BTreeMap<String, Value>) -> Result<Vec<String>, PackError> {
    let mut ordered: Vec<String> = Vec::new();
    let mut pending: Vec<&String> = decisions.keys().collect();
    while !pending.is_empty() {
        let ready: Vec<&String> = pending
            .iter()
            .copied()
            .filter(|key| {
                calls(&decisions[*key]).iter().all(|called| {
                    !decisions.contains_key(called) || ordered.contains(called) || called == *key
                })
            })
            .collect();
        if ready.is_empty() {
            return Err(PackError::Invalid(
                "decisions call each other in a loop".into(),
            ));
        }
        pending.retain(|key| !ready.contains(key));
        ordered.extend(ready.into_iter().cloned());
    }
    Ok(ordered)
}

fn collect(dir: &Path, prefix: &str, files: &mut Files) -> Result<(), PackError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|err| PackError::Invalid(format!("{}: {err}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|err| PackError::Invalid(err.to_string()))?;
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            collect(&path, &name, files)?;
        } else {
            files.insert(
                name,
                std::fs::read(&path).map_err(|err| PackError::Invalid(err.to_string()))?,
            );
        }
    }
    Ok(())
}

fn json_file<T: serde::de::DeserializeOwned>(files: &Files, name: &str) -> Result<T, PackError> {
    let bytes = files
        .get(name)
        .ok_or_else(|| PackError::Invalid(format!("{name} is missing")))?;
    serde_json::from_slice(bytes).map_err(|err| PackError::Invalid(format!("{name}: {err}")))
}

fn pretty<T: Serialize>(value: &T) -> Result<Vec<u8>, PackError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|err| PackError::Invalid(err.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn zip_error(err: zip::result::ZipError) -> PackError {
    PackError::Invalid(err.to_string())
}

#[cfg(test)]
mod tests {
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
        assert_eq!(pack.manifest.decisions, ["affordability", "scorecard"]);
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
            entries.push((format!("sme-treasury/{}", entry.name()), bytes));
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
            ["affordability", "scorecard"]
        );
        let mut reversed = pack.clone();
        reversed.manifest.decisions.reverse();
        assert!(reversed
            .check()
            .unwrap_err()
            .to_string()
            .contains("calls affordability"));

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
}
