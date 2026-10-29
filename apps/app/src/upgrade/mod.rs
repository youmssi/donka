//! `donka-admin upgrade` (DNK-64): moves an installation to another release, run by the
//! installed version so that only images it can verify reach the server.
//!
//! 1. Verify: the new images' cosign signatures, with the installation's `cosign.pub`.
//! 2. Check compatibility: the new release's `upgrade.json` against the installed versions.
//! 3. Back up the database, the release and wiring volumes, and the installation's files.
//! 4. Upgrade the Runtime, then Studio, each followed by the smoke test.
//! 5. If a step fails, restore the backup and the previous images, and check them.
//!
//! Every run writes a report under `upgrades/`; refusals change nothing.

pub mod auto;
pub mod compat;
pub mod docker;
pub mod registry;
pub mod signature;

use chrono::Utc;
use docker::{Engine, Helper, INSTALL};
use registry::{ImageRef, Registry};
use serde::Serialize;
use signature::PublicKey;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The image with `docker compose` the upgrade runs it from; the compose file's `tools`
/// profile names the same one, so pulls and the offline bundle include it.
pub const COMPOSE_CLI: &str = "docker:28.5-cli";
/// The compose service that runs unattended upgrades (`upgrade::auto`).
pub const UPGRADER: &str = "auto-upgrade";
/// Where Studio's image carries its compose file (the one `donka-admin init` writes).
pub const COMPOSE_PATH: &str = "/etc/donka/docker-compose.yml";
pub(crate) const DEFAULT_REGISTRY: &str = "ghcr.io/youmssi";
const ENV_FILES: [&str; 3] = [".env", "runtime.env", "docker-compose.yml"];
/// How long a service has to answer after it is recreated.
const START_TIMEOUT: Duration = Duration::from_secs(180);

pub struct Options {
    /// The installation folder, as the upgrade's container sees it.
    pub dir: PathBuf,
    pub studio: String,
    /// `None` keeps the installed Runtime.
    pub runtime: Option<String>,
    pub key: PathBuf,
    pub credentials: Option<(String, String)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub started_at: String,
    pub finished_at: String,
    pub from: Versions,
    pub to: Versions,
    pub outcome: Outcome,
    pub steps: Vec<Step>,
    /// Where the backup is, once taken.
    pub backup: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Versions {
    pub studio: String,
    pub runtime: String,
}

#[derive(Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// The installation runs the new versions.
    Upgraded,
    /// Nothing was changed: an image did not verify, or the release refuses this installation.
    Refused,
    /// A step failed after changes began; the previous versions and data are back.
    RolledBack,
    /// A step failed and so did the restore: the report and the backup say where things are.
    RestoreFailed,
}

#[derive(Serialize)]
pub struct Step {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

impl Report {
    fn step(&mut self, name: &str, result: Result<String, String>) -> Result<(), String> {
        let ok = result.is_ok();
        let detail = match &result {
            Ok(detail) | Err(detail) => detail.clone(),
        };
        println!("{} {name}: {detail}", if ok { "ok  " } else { "FAIL" });
        self.steps.push(Step {
            name: name.to_owned(),
            ok,
            detail,
        });
        result.map(|_| ())
    }

    pub fn text(&self) -> String {
        let mut text = format!(
            "Donka upgrade, {} to {}\nStudio {} -> {}, Runtime {} -> {}\nOutcome: {:?}\n",
            self.started_at,
            self.finished_at,
            self.from.studio,
            self.to.studio,
            self.from.runtime,
            self.to.runtime,
            self.outcome
        );
        if let Some(backup) = &self.backup {
            text.push_str(&format!("Backup: {backup}\n"));
        }
        text.push('\n');
        for step in &self.steps {
            let mark = if step.ok { "ok  " } else { "FAIL" };
            text.push_str(&format!("{mark} {}: {}\n", step.name, step.detail));
        }
        text
    }
}

/// Runs the upgrade and writes its report; `Err` only when no report could be started.
pub async fn run(options: Options) -> Result<Report, String> {
    let env = read_env(&options.dir.join(".env"))?;
    let installed = Versions {
        studio: env_value(&env, "DONKA_VERSION")
            .ok_or(".env has no DONKA_VERSION: run the upgrade in the installation folder")?,
        runtime: env_value(&env, "DONKA_RUNTIME_VERSION")
            .ok_or(".env has no DONKA_RUNTIME_VERSION")?,
    };
    let target = Versions {
        studio: options.studio.clone(),
        runtime: options
            .runtime
            .clone()
            .unwrap_or_else(|| installed.runtime.clone()),
    };
    let started = Utc::now();
    let stamp = started.format("%Y%m%dT%H%M%SZ").to_string();
    let mut report = Report {
        started_at: started.to_rfc3339(),
        finished_at: String::new(),
        from: installed.clone(),
        to: target.clone(),
        outcome: Outcome::Refused,
        steps: Vec::new(),
        backup: None,
    };
    let result = upgrade(&options, &env, &installed, &target, &stamp, &mut report).await;
    report.outcome = result;
    report.finished_at = Utc::now().to_rfc3339();
    let name = format!("{stamp}-{}-to-{}", installed.studio, target.studio);
    let dir = options.dir.join("upgrades");
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot write {}: {err}", dir.display()))?;
    let json = serde_json::to_vec_pretty(&report).map_err(|err| err.to_string())?;
    std::fs::write(dir.join(format!("{name}.json")), json).map_err(|err| err.to_string())?;
    std::fs::write(dir.join(format!("{name}.txt")), report.text())
        .map_err(|err| err.to_string())?;
    println!("\nReport: upgrades/{name}.txt");
    Ok(report)
}

async fn upgrade(
    options: &Options,
    env: &str,
    installed: &Versions,
    target: &Versions,
    stamp: &str,
    report: &mut Report,
) -> Outcome {
    let registry_prefix =
        env_value(env, "DONKA_REGISTRY").unwrap_or_else(|| DEFAULT_REGISTRY.to_owned());
    let studio = format!("{registry_prefix}/donka-studio");
    let runtime = format!("{registry_prefix}/donka-runtime");

    // Nothing changes until the images verify and the release accepts this installation.
    let prepared = async {
        let key = std::fs::read_to_string(&options.key)
            .map_err(|err| {
                format!(
                    "cannot read {} ({err}): save Donka's cosign.pub in the installation folder",
                    options.key.display()
                )
            })
            .and_then(|text| PublicKey::from_pem(&text))?;
        let engine = Engine::connect().await?;
        engine.join_network().await?;
        let registry = Registry::new(
            options.credentials.clone(),
            engine.insecure_registries().await?,
        )?;
        let mut images = vec![(studio.clone(), target.studio.clone())];
        if target.runtime != installed.runtime {
            images.push((runtime.clone(), target.runtime.clone()));
        }
        Ok::<_, String>((key, engine, registry, images))
    }
    .await;
    let (key, engine, registry, images) = match prepared {
        Ok(prepared) => prepared,
        Err(err) => {
            let _ = report.step("prepare", Err(err));
            return Outcome::Refused;
        }
    };
    for (name, tag) in &images {
        let verified = async {
            let image = ImageRef::parse(&format!("{name}:{tag}"))?;
            let digest = registry.digest(&image).await?;
            registry.verify(&image, &digest, &key).await?;
            engine
                .pull_verified(name, &digest, tag, options.credentials.clone())
                .await?;
            Ok(format!(
                "{image} is {digest}, signed with the installation's key"
            ))
        }
        .await;
        if report.step("verify the image", verified).is_err() {
            return Outcome::Refused;
        }
    }
    let new_studio = format!("{studio}:{}", target.studio);
    let compatible = async {
        let rules = compat::parse(&engine.read_file(&new_studio, compat::PATH).await?)?;
        match rules.check(&installed.studio, &target.runtime) {
            compat::Verdict::Compatible => Ok(format!(
                "Studio {} upgrades from {} and works with Runtime {}",
                target.studio, installed.studio, target.runtime
            )),
            compat::Verdict::Unchecked(why) => Ok(format!("not checked: {why}")),
            compat::Verdict::Incompatible(why) => Err(why),
        }
    }
    .await;
    if report.step("check compatibility", compatible).is_err() {
        return Outcome::Refused;
    }
    let compose = match engine.read_file(&new_studio, COMPOSE_PATH).await {
        Ok(compose) => compose,
        Err(err) => {
            let _ = report.step("read the new compose file", Err(err));
            return Outcome::Refused;
        }
    };
    let previous = Previous {
        app: engine.service_image("app").await.ok().flatten(),
        runtime: engine.service_image("runtime").await.ok().flatten(),
        postgres: engine.service_image("postgres").await.ok().flatten(),
    };
    let Some(postgres) = previous.postgres.clone() else {
        let _ = report.step("back up", Err("the installation's postgres container is not there: start it with docker compose up -d".to_owned()));
        return Outcome::Refused;
    };
    for helper in [COMPOSE_CLI] {
        if let Err(err) = ensure_image(&engine, helper).await {
            let _ = report.step("prepare", Err(err));
            return Outcome::Refused;
        }
    }

    let backup = format!("backups/{stamp}");
    let tools = Tools {
        engine: &engine,
        dir: &options.dir,
        postgres: &postgres,
        password: env_value(env, "DONKA_POSTGRES_PASSWORD").unwrap_or_default(),
    };
    if report
        .step("back up", tools.back_up(&backup).await)
        .is_err()
    {
        return Outcome::Refused;
    }
    report.backup = Some(backup.clone());

    let base_path = env_value(env, "DONKA_API_BASE_PATH").unwrap_or_else(|| "/api/v1".to_owned());
    let changed = async {
        std::fs::write(options.dir.join("docker-compose.yml"), &compose)
            .map_err(|err| err.to_string())?;
        if target.runtime != installed.runtime {
            set_env(
                &options.dir.join(".env"),
                "DONKA_RUNTIME_VERSION",
                &target.runtime,
            )?;
            tools.compose(&["up", "-d", "--no-deps", "runtime"]).await?;
            let image = engine
                .image_id(&format!("{runtime}:{}", target.runtime))
                .await?;
            smoke_runtime(&engine, image.as_deref()).await?;
        }
        Ok::<_, String>(format!("Runtime {}", target.runtime))
    }
    .await;
    let upgraded = if report.step("upgrade the Runtime", changed).is_ok() {
        let studio_done = async {
            set_env(&options.dir.join(".env"), "DONKA_VERSION", &target.studio)?;
            tools.start_all().await?;
            let image = engine.image_id(&new_studio).await?;
            smoke_studio(&engine, &base_path, image.as_deref()).await?;
            smoke_runtime(&engine, None).await?;
            Ok::<_, String>(format!(
                "Studio {}; Studio ready, the Runtime healthy",
                target.studio
            ))
        }
        .await;
        report.step("upgrade Studio", studio_done).is_ok()
    } else {
        false
    };
    if upgraded {
        return Outcome::Upgraded;
    }

    let restored = async {
        tools.restore(&backup).await?;
        smoke_studio(&engine, &base_path, previous.app.as_deref()).await?;
        smoke_runtime(&engine, previous.runtime.as_deref()).await?;
        Ok::<_, String>(format!(
            "Studio {} and Runtime {} with the data of {backup}",
            installed.studio, installed.runtime
        ))
    }
    .await;
    match report.step("restore the previous versions", restored) {
        Ok(()) => Outcome::RolledBack,
        Err(_) => Outcome::RestoreFailed,
    }
}

struct Previous {
    app: Option<String>,
    runtime: Option<String>,
    postgres: Option<String>,
}

/// The helper containers the upgrade runs, with the installation folder mounted.
struct Tools<'a> {
    engine: &'a Engine,
    dir: &'a Path,
    postgres: &'a str,
    password: String,
}

impl Tools<'_> {
    fn install_bind(&self) -> String {
        format!("{}:{INSTALL}", self.engine.host_dir)
    }

    /// `docker compose <args>` in the installation folder.
    async fn compose(&self, args: &[&str]) -> Result<(), String> {
        let mut cmd = vec!["docker".to_owned(), "compose".to_owned()];
        cmd.extend(args.iter().map(|arg| (*arg).to_owned()));
        self.engine.run(compose_helper(self.engine, cmd)).await
    }

    /// Starts every service but the unattended upgrader, which may be the one running this
    /// upgrade: it is recreated last, once the upgrade has reported.
    async fn start_all(&self) -> Result<(), String> {
        let script = format!(
            "docker compose up -d $(docker compose config --services | grep -vx {UPGRADER})"
        );
        let cmd = vec!["sh".to_owned(), "-ec".to_owned(), script];
        self.engine.run(compose_helper(self.engine, cmd)).await
    }

    async fn postgres_shell(&self, script: String) -> Result<(), String> {
        self.engine
            .run(Helper {
                image: self.postgres.to_owned(),
                cmd: vec!["sh".into(), "-ec".into(), script],
                binds: vec![self.install_bind()],
                env: vec![format!("PGPASSWORD={}", self.password)],
                user: Some(self.engine.user.clone()),
                network: true,
                ..Default::default()
            })
            .await
    }

    /// The volumes' files are owned by Studio's user and the token is private: tar runs as
    /// root, then gives the archive to the operator.
    async fn volumes_shell(&self, script: String) -> Result<(), String> {
        self.engine
            .run(Helper {
                image: self.postgres.to_owned(),
                cmd: vec!["sh".into(), "-ec".into(), script],
                binds: vec![
                    self.install_bind(),
                    format!("{}_releases:/volumes/releases", docker::PROJECT),
                    format!("{}_wiring:/volumes/wiring", docker::PROJECT),
                ],
                user: Some("0".to_owned()),
                ..Default::default()
            })
            .await
    }

    async fn back_up(&self, backup: &str) -> Result<String, String> {
        let folder = self.dir.join(backup);
        std::fs::create_dir_all(&folder).map_err(|err| format!("cannot create {backup}: {err}"))?;
        for file in ENV_FILES {
            let from = self.dir.join(file);
            if from.exists() {
                std::fs::copy(&from, folder.join(file))
                    .map_err(|err| format!("cannot copy {file}: {err}"))?;
            }
        }
        self.postgres_shell(format!(
            "pg_dump -h postgres -U donka -Fc -f {INSTALL}/{backup}/donka.dump donka"
        ))
        .await?;
        let owner = &self.engine.user;
        self.volumes_shell(format!(
            "tar -C /volumes -czf {INSTALL}/{backup}/volumes.tar.gz releases wiring && \
             chown {owner} {INSTALL}/{backup}/volumes.tar.gz"
        ))
        .await?;
        Ok(format!(
            "{backup}: database, volumes, .env, runtime.env, docker-compose.yml"
        ))
    }

    async fn restore(&self, backup: &str) -> Result<(), String> {
        let folder = self.dir.join(backup);
        for file in ENV_FILES {
            let from = folder.join(file);
            if from.exists() {
                std::fs::copy(&from, self.dir.join(file))
                    .map_err(|err| format!("cannot restore {file}: {err}"))?;
            }
        }
        self.compose(&["stop", "app", "runtime"]).await?;
        self.postgres_shell(format!(
            "psql -h postgres -U donka -d postgres -v ON_ERROR_STOP=1 \
               -c 'DROP DATABASE IF EXISTS donka WITH (FORCE)' -c 'CREATE DATABASE donka OWNER donka' && \
             pg_restore -h postgres -U donka -d donka --no-owner {INSTALL}/{backup}/donka.dump"
        ))
        .await?;
        self.volumes_shell(format!(
            "find /volumes/releases /volumes/wiring -mindepth 1 -delete && \
             tar -C /volumes -xzf {INSTALL}/{backup}/volumes.tar.gz"
        ))
        .await?;
        self.start_all().await
    }
}

/// Recreates the `auto-upgrade` service with the release `.env` now names. The helper outlives
/// the service it replaces, which may be the caller.
pub async fn restart_upgrader(engine: &Engine) -> Result<(), String> {
    let cmd = ["docker", "compose", "up", "-d", UPGRADER]
        .map(String::from)
        .to_vec();
    engine.start_detached(compose_helper(engine, cmd)).await
}

/// A helper running `cmd` with `docker compose` in the installation folder, at its host path so
/// that relative paths in the compose file mean the same thing.
fn compose_helper(engine: &Engine, cmd: Vec<String>) -> Helper {
    let host = &engine.host_dir;
    Helper {
        image: COMPOSE_CLI.to_owned(),
        cmd,
        binds: vec![
            "/var/run/docker.sock:/var/run/docker.sock".to_owned(),
            format!("{host}:{host}"),
        ],
        working_dir: Some(host.clone()),
        user: Some("0".to_owned()),
        ..Default::default()
    }
}

async fn ensure_image(engine: &Engine, image: &str) -> Result<(), String> {
    if engine.image_id(image).await?.is_some() {
        return Ok(());
    }
    let (name, tag) = image
        .rsplit_once(':')
        .ok_or_else(|| format!("{image} has no tag"))?;
    engine.pull_tag(name, tag).await
}

/// Studio answers ready (its database reachable and migrated) and runs `image`.
async fn smoke_studio(engine: &Engine, base_path: &str, image: Option<&str>) -> Result<(), String> {
    wait_for(&format!("http://app:8080{base_path}/ready"), "Studio").await?;
    runs(engine, "app", image).await
}

async fn smoke_runtime(engine: &Engine, image: Option<&str>) -> Result<(), String> {
    wait_for("http://runtime:8080/api/health", "the Runtime").await?;
    runs(engine, "runtime", image).await
}

async fn runs(engine: &Engine, service: &str, image: Option<&str>) -> Result<(), String> {
    let Some(expected) = image else { return Ok(()) };
    let running = engine.service_image(service).await?;
    if running.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err(format!("{service} runs {running:?}, not {expected}"))
    }
}

async fn wait_for(url: &str, what: &str) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| err.to_string())?;
    let deadline = tokio::time::Instant::now() + START_TIMEOUT;
    let mut last = String::new();
    while tokio::time::Instant::now() < deadline {
        match http.get(url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => last = format!("answered {}", response.status()),
            Err(err) => last = err.to_string(),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(format!(
        "{what} did not answer {url} within {} s ({last})",
        START_TIMEOUT.as_secs()
    ))
}

fn read_env(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| {
        format!(
            "cannot read {} ({err}): run the upgrade in the installation folder",
            path.display()
        )
    })
}

/// The value of `KEY=value` in an env file, ignoring comments.
fn env_value(env: &str, key: &str) -> Option<String> {
    env.lines()
        .filter_map(|line| line.trim().split_once('='))
        .find(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Sets `key` in the env file at `path`, keeping every other line as it is.
fn set_env(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let env = read_env(path)?;
    std::fs::write(path, with_env(&env, key, value))
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn with_env(env: &str, key: &str, value: &str) -> String {
    let mut found = false;
    let mut lines: Vec<String> = env
        .lines()
        .map(|line| match line.split_once('=') {
            Some((name, _)) if name.trim() == key => {
                found = true;
                format!("{key}={value}")
            }
            _ => line.to_owned(),
        })
        .collect();
    if !found {
        lines.push(format!("{key}={value}"));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_values_are_read_and_replaced_in_place() {
        let env =
            "# Images\nDONKA_VERSION=1.4.0\nDONKA_RUNTIME_VERSION=1.2.0\n# DONKA_REGISTRY=x\n";
        assert_eq!(env_value(env, "DONKA_VERSION").as_deref(), Some("1.4.0"));
        assert_eq!(env_value(env, "DONKA_REGISTRY"), None);
        let changed = with_env(env, "DONKA_VERSION", "1.5.0");
        assert_eq!(
            changed,
            "# Images\nDONKA_VERSION=1.5.0\nDONKA_RUNTIME_VERSION=1.2.0\n# DONKA_REGISTRY=x\n"
        );
        assert!(with_env(env, "DONKA_REGISTRY", "r")
            .ends_with("# DONKA_REGISTRY=x\nDONKA_REGISTRY=r\n"));
    }

    #[test]
    fn the_compose_file_names_the_tool_image_the_upgrade_runs() {
        assert!(crate::install::COMPOSE.contains(&format!("image: {COMPOSE_CLI}")));
    }

    #[test]
    fn a_report_reads_as_a_change_record() {
        let mut report = Report {
            started_at: "2026-10-29T10:00:00Z".into(),
            finished_at: "2026-10-29T10:03:00Z".into(),
            from: Versions {
                studio: "1.4.0".into(),
                runtime: "1.2.0".into(),
            },
            to: Versions {
                studio: "1.5.0".into(),
                runtime: "1.2.0".into(),
            },
            outcome: Outcome::RolledBack,
            steps: Vec::new(),
            backup: Some("backups/20261029T100000Z".into()),
        };
        let _ = report.step("back up", Ok("done".into()));
        let _ = report.step("upgrade Studio", Err("Studio did not answer".into()));
        let text = report.text();
        assert!(text.contains("Studio 1.4.0 -> 1.5.0"), "{text}");
        assert!(text.contains("Outcome: RolledBack"), "{text}");
        assert!(
            text.contains("FAIL upgrade Studio: Studio did not answer"),
            "{text}"
        );
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["outcome"], "rolledBack");
        assert_eq!(json["steps"][1]["ok"], false);
    }
}
