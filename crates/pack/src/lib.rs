//! Packs (DNK-43): start a project from a pack in the catalogue or a pack file, copy a
//! project to start another product, and export a project as a pack another team or
//! installation imports.
//!
//! A copy carries decisions (the drafts, or the versions a release froze), test scenarios,
//! each decision's input contract (it is part of the graph) and the decision-log settings.
//! It never carries releases, deployments, tokens, decision records or members: the person
//! who makes it is its only owner. Nothing links a copy back to where it came from; its
//! audit log opens with that origin.
//!
//! Each step goes through the public API of the module that owns it, so a pack is checked
//! in full first: once the project exists, only a database failure can stop half-way.

mod format;

pub use format::{
    calls, dependency_order, Guides, Localized, LogSettings, Manifest, Pack, Scenario, FORMAT,
    MAX_FILE_BYTES,
};

use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_decision::{DecisionError, Decisions, TestSummary};
use donka_decision_log::{DecisionLog, DecisionLogError};
use donka_identity::{Locale, User};
use donka_project::{Access, Project, ProjectError, Projects, Role};
use donka_release::{ReleaseError, Releases};
use donka_shared::clock::Clock;
use donka_shared::page::PageRequest;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// The pack cannot be used; why, in words.
    #[error("{0}")]
    Invalid(String),
    #[error("a pack file is at most {} MB", MAX_FILE_BYTES / 1024 / 1024)]
    TooLarge,
    #[error("no pack with this key in the catalogue")]
    NotFound,
    /// Only administrators create projects.
    #[error("only administrators can create projects")]
    NotAdministrator,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Decision(#[from] DecisionError),
    #[error(transparent)]
    DecisionLog(#[from] DecisionLogError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The packs an installation offers (`DONKA_PACKS_DIR`), read once at start.
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    packs: Vec<Pack>,
}

impl Catalogue {
    /// Every `<dir>/<key>/pack.json` pack; one that cannot be used stops Studio, naming it.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        let mut packs = Vec::new();
        for entry in entries {
            let path = entry.map_err(|err| err.to_string())?.path();
            if path.join("pack.json").is_file() {
                packs.push(
                    Pack::from_dir(&path).map_err(|err| format!("{}: {err}", path.display()))?,
                );
            }
        }
        packs.sort_by(|a, b| a.manifest.key.cmp(&b.manifest.key));
        Ok(Self { packs })
    }

    pub fn packs(&self) -> &[Pack] {
        &self.packs
    }

    pub fn get(&self, key: &str) -> Result<&Pack, PackError> {
        self.packs
            .iter()
            .find(|pack| pack.manifest.key == key)
            .ok_or(PackError::NotFound)
    }
}

/// What a copy is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Each decision's current draft.
    Drafts,
    /// The versions a release froze.
    Release(Uuid),
}

/// A project made from a pack or another project.
#[derive(Debug, Clone)]
pub struct Imported {
    pub project: Project,
    /// How the scenarios went on the versions the copy started with.
    pub tests: TestSummary,
}

#[derive(Clone)]
pub struct Packs {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    projects: Projects,
    decisions: Decisions,
    decision_log: DecisionLog,
    releases: Releases,
}

impl Packs {
    pub fn new(
        pool: PgPool,
        clock: Arc<dyn Clock>,
        projects: Projects,
        decisions: Decisions,
        decision_log: DecisionLog,
        releases: Releases,
    ) -> Self {
        Self {
            pool,
            clock,
            projects,
            decisions,
            decision_log,
            releases,
        }
    }

    /// A new project from `pack`, under `key` and `name` (administrators).
    pub async fn import(
        &self,
        actor: &User,
        pack: &Pack,
        key: &str,
        name: &str,
    ) -> Result<Imported, PackError> {
        let manifest = &pack.manifest;
        let origin = json!({
            "kind": "pack",
            "key": manifest.key,
            "name": manifest.name.get(lang(actor)),
            "version": manifest.version,
        });
        let message = match &manifest.version {
            Some(version) => format!("Created from pack {} {version}", manifest.name.en),
            None => format!("Created from pack {}", manifest.name.en),
        };
        self.create(
            actor,
            pack,
            key,
            name,
            manifest.description.get(lang(actor)),
            origin,
            &message,
        )
        .await
    }

    /// A new project with what `source` holds (administrators who are members of it).
    pub async fn duplicate(
        &self,
        actor: &User,
        access: &Access,
        from: Source,
        key: &str,
        name: &str,
    ) -> Result<Imported, PackError> {
        if !actor.is_admin {
            return Err(PackError::NotAdministrator);
        }
        let (pack, project, release) = self.collect(access, from).await?;
        let origin = json!({
            "kind": "project",
            "key": project.key,
            "name": project.name,
            "release": release,
        });
        let message = format!("Duplicated from project {}", project.key);
        let imported = self
            .create(
                actor,
                &pack,
                key,
                name,
                &project.description,
                origin,
                &message,
            )
            .await?;
        self.record(
            access,
            Action::ProjectDuplicated,
            json!({
                "toKey": imported.project.key,
                "toName": imported.project.name,
                "release": release,
            }),
        )
        .await?;
        Ok(imported)
    }

    /// The project as a pack (owners). The download is audited in the project.
    pub async fn export(&self, access: &Access, from: Source) -> Result<Pack, PackError> {
        access.require(Role::Owner)?;
        let (pack, _, release) = self.collect(access, from).await?;
        self.record(
            access,
            Action::ProjectExported,
            json!({
                "release": release,
                "decisions": pack.manifest.decisions.len(),
                "scenarios": pack.scenarios.len(),
            }),
        )
        .await?;
        Ok(pack)
    }

    /// The pack a project's `from` makes, with the project and the release version it used.
    async fn collect(
        &self,
        access: &Access,
        from: Source,
    ) -> Result<(Pack, Project, Option<String>), PackError> {
        let project = self.projects.get(access).await?;
        let summaries = self.decisions.list(access).await?;
        let mut decisions = BTreeMap::new();
        let mut keys_by_id = BTreeMap::new();
        let release = match from {
            Source::Drafts => {
                for summary in summaries {
                    let decision = self.decisions.get(access, summary.id).await?;
                    keys_by_id.insert(summary.id, summary.key.clone());
                    decisions.insert(summary.key, decision.content);
                }
                None
            }
            Source::Release(id) => {
                let release = self.releases.get(access, id).await?;
                for frozen in release.decisions {
                    let version = self
                        .decisions
                        .version(access, frozen.decision_id, frozen.version)
                        .await?;
                    keys_by_id.insert(frozen.decision_id, frozen.key.clone());
                    decisions.insert(frozen.key, version.content);
                }
                Some(release.summary.version.to_string())
            }
        };
        if decisions.is_empty() {
            return Err(PackError::Invalid(
                "the project has no decision to copy".into(),
            ));
        }

        let mut scenarios = Vec::new();
        let mut offset = 0;
        loop {
            let page = self
                .decisions
                .scenarios(access, None, PageRequest { limit: 500, offset })
                .await?;
            let count = page.items.len() as i64;
            for scenario in page.items {
                // A release's copy keeps the scenarios of the decisions it froze.
                if let Some(key) = keys_by_id.get(&scenario.decision_id) {
                    scenarios.push(Scenario {
                        decision: key.clone(),
                        name: scenario.name,
                        input: scenario.input,
                        expected: scenario.expected,
                        match_mode: scenario.match_mode.as_str().to_owned(),
                    });
                }
            }
            offset += count;
            if count == 0 || offset >= page.total {
                break;
            }
        }

        let settings = self.decision_log.settings(access).await?;
        let pack = Pack {
            manifest: Manifest {
                format: FORMAT,
                key: project.key.clone(),
                name: Localized::same(&project.name),
                description: Localized::same(&project.description),
                version: release.clone(),
                market: None,
                currency: None,
                entry: None,
                decisions: dependency_order(&decisions)?,
                decision_log: LogSettings {
                    outcome_field: settings.outcome_field,
                    redacted_fields: settings.redacted_fields,
                },
            },
            decisions,
            scenarios,
            guides: Guides::default(),
        };
        pack.check()?;
        Ok((pack, project, release))
    }

    /// The project, its decisions and scenarios, then a first version of each decision, which
    /// runs the scenarios.
    #[allow(clippy::too_many_arguments)]
    async fn create(
        &self,
        actor: &User,
        pack: &Pack,
        key: &str,
        name: &str,
        description: &str,
        origin: Value,
        message: &str,
    ) -> Result<Imported, PackError> {
        if !actor.is_admin {
            return Err(PackError::NotAdministrator);
        }
        pack.check()?;
        donka_project::check_new(key, name, description)?;

        let project = self
            .projects
            .create_from(actor.id, key, name, description, Some(origin))
            .await?;
        let access = self.projects.access(actor.id, project.id).await?;

        let mut created = Vec::new();
        for key in &pack.manifest.decisions {
            let decision = self
                .decisions
                .create(&access, key, Some(pack.decisions[key].clone()))
                .await?;
            created.push((decision.summary.id, decision.summary.revision));
        }
        let ids: BTreeMap<&String, Uuid> = pack
            .manifest
            .decisions
            .iter()
            .zip(created.iter().map(|(id, _)| *id))
            .collect();
        if !pack.manifest.decision_log.is_default() {
            self.decision_log
                .update_settings(&access, pack.manifest.decision_log.change())
                .await?;
        }
        // Scenarios before versions: saving a version runs them.
        for scenario in &pack.scenarios {
            self.decisions
                .create_scenario(&access, ids[&scenario.decision], scenario.fields()?)
                .await?;
        }
        // Each save runs every scenario of the project; the last one, once every decision has
        // a version, tells how the copy does.
        let mut tests = TestSummary::default();
        for (id, revision) in created {
            tests = self
                .decisions
                .save_version(&access, id, revision, message)
                .await?
                .summary
                .tests;
        }
        Ok(Imported { project, tests })
    }

    async fn record(
        &self,
        access: &Access,
        action: Action,
        details: Value,
    ) -> Result<(), PackError> {
        let mut tx = self.pool.begin().await?;
        donka_audit::record(
            &mut tx,
            Event::new(self.clock.now(), Some(access.user_id()), action)
                .in_project(access.project_id())
                .with_details(details),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn lang(user: &User) -> &'static str {
    match user.locale {
        Locale::Fr => "fr",
        Locale::En => "en",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_holds_every_pack_of_the_repository() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs");
        let catalogue = Catalogue::load(&dir).unwrap();
        let keys: Vec<&str> = catalogue
            .packs()
            .iter()
            .map(|pack| pack.manifest.key.as_str())
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{keys:?}");
        assert!(keys.contains(&"retail-credit"));
        assert!(catalogue.get("retail-credit").is_ok());
        assert!(matches!(catalogue.get("nope"), Err(PackError::NotFound)));
        assert!(Catalogue::load(&dir.join("missing")).is_err());
    }

    /// Each decision's input contract matches what its rules read (DNK-37).
    #[test]
    fn every_contract_of_the_catalogue_matches_its_rules() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs");
        for pack in Catalogue::load(&dir).unwrap().packs() {
            for (key, graph) in &pack.decisions {
                let schema = donka_engine::contract::input_schema(graph).unwrap();
                if let Some(schema) = schema {
                    let warnings = donka_engine::contract::check(&pack.decisions, key, &schema);
                    assert!(warnings.is_empty(), "{key}: {warnings:?}");
                }
            }
        }
    }
}
