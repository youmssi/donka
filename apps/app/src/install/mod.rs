//! `donka-admin init` (DNK-64, DNK-74): prepares an installation from the published images,
//! with nothing but Docker on the server.
//!
//! `donka.yaml` holds the installation's answers and no secret. From it, init writes the files
//! Docker Compose reads: `.env` (Studio's settings), `runtime.env` (the staging Runtime's) and
//! `docker-compose.yml`, and the secrets as files in `secrets/`, which containers read through
//! the `*_FILE` settings. Running init again with `donka.yaml` writes the same files; a secret
//! that exists is never overwritten, and none is ever printed.

pub mod assistant;

use crate::upgrade::auto::{Role, Window};
use crate::upgrade::UPGRADER;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use donka_licence::Module;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The installation's compose file: Studio, the staging Runtime, PostgreSQL and Mailpit.
pub const COMPOSE: &str = include_str!("../../install/docker-compose.yml");
/// The installation's answers, next to the files written from them.
pub const CONFIG: &str = "donka.yaml";
/// The folder of secret files, mounted read-only at `/run/secrets` in the containers.
pub const SECRETS: &str = "secrets";
const MOUNTED: &str = "/run/secrets";

// The secrets, one file each.
pub const POSTGRES_PASSWORD: &str = "postgres_password";
const DATABASE_URL: &str = "database_url";
const DECISION_LOG_KEY: &str = "decision_log_key";
const SECOND_STEP_KEY: &str = "second_step_key";
const PERSON_INDEX_KEY: &str = "person_index_key";
const SMTP_URL: &str = "smtp_url";
const STORAGE_ACCESS_KEY_ID: &str = "storage_access_key_id";
const STORAGE_SECRET_ACCESS_KEY: &str = "storage_secret_access_key";
const EXPLAIN_API_KEY: &str = "explain_api_key";
const LICENCE: &str = "licence.json";
/// The demo's connector secret: the key the Runtime sends to the fake bureau.
const DEMO_CONNECTOR_SECRET: &str = "BUREAU_API_KEY";

/// `donka.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    /// The assistant's language, and Studio's for new accounts: `en` or `fr`.
    pub language: Language,
    /// Where people reach Studio. Behind HTTPS, sessions use secure cookies.
    pub public_url: String,
    /// The first administrator, created on the empty database.
    pub administrator: String,
    /// What the installation is: production, staging or demo. Production never upgrades itself.
    pub role: Role,
    /// Who signs in with a second step: none, privileged (administrators and owners) or all.
    pub second_step: SecondStep,
    pub images: Images,
    pub storage: Storage,
    pub mail: Mail,
    /// The language model that explains decisions, chosen by the customer.
    pub explanations: Explanations,
    /// Donka's licence file, kept in `secrets/licence.json`; without it, evaluation.
    #[serde(default)]
    pub licence: bool,
    /// The retail-credit pack released to staging at the first start, with a fake bureau.
    #[serde(default)]
    pub demo: bool,
    /// No internet access: Studio never checks for new releases.
    #[serde(default)]
    pub offline: bool,
    /// The weekly window of unattended upgrades (staging and demo only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_upgrade: Option<Window>,
    /// The optional modules turned on (DNK-74): `explain`, `connectors`, `analysis`, within what
    /// the licence includes. Every module the licence includes when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<BTreeSet<Module>>,
    /// The connector secrets the Runtime reads (`DONKA_SECRET_<NAME>`), each in
    /// `secrets/connector_<name>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connector_secrets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Fr,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SecondStep {
    #[default]
    None,
    Privileged,
    All,
}

impl SecondStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Privileged => "privileged",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Images {
    pub studio: String,
    pub runtime: String,
    /// A mirror of `ghcr.io/youmssi`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}

/// Where releases are published for the Runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Storage {
    /// A volume Studio and the staging Runtime share, on this server.
    #[default]
    Disk,
    /// The customer's S3 (or SILO): its keys in `secrets/storage_access_key_id` and
    /// `secrets/storage_secret_access_key`.
    S3 {
        bucket: String,
        region: String,
        /// For an S3-compatible server; AWS without it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Mail {
    /// Mailpit catches every email (http://localhost:8025).
    #[default]
    Mailpit,
    /// The customer's server: its URL, with credentials, in `secrets/smtp_url`.
    Smtp { from: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Explanations {
    #[default]
    Off,
    /// A Messages API endpoint; its key in `secrets/explain_api_key`.
    Anthropic { url: String, model: String },
    /// An OpenAI-compatible endpoint; its key in `secrets/explain_api_key`.
    Openai { url: String, model: String },
}

impl Installation {
    /// The defaults for a first look on this machine, with the images of this Studio's own
    /// version: a release pins itself and the latest Runtime; any other build uses `edge`.
    pub fn for_version(version: &str) -> Self {
        let released = !version.is_empty()
            && version
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
        Self {
            language: Language::En,
            public_url: "http://localhost:8080".to_owned(),
            administrator: "admin@donka.local".to_owned(),
            role: Role::Production,
            second_step: SecondStep::None,
            images: Images {
                studio: if released { version } else { "edge" }.to_owned(),
                runtime: if released { "latest" } else { "edge" }.to_owned(),
                registry: None,
            },
            storage: Storage::Disk,
            mail: Mail::Mailpit,
            explanations: Explanations::Off,
            licence: false,
            demo: false,
            offline: false,
            auto_upgrade: None,
            modules: None,
            connector_secrets: Vec::new(),
        }
    }

    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(CONFIG);
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        serde_yaml_ng::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
    }

    fn validate(&self) -> Result<(), String> {
        if self.auto_upgrade.is_some() && !self.role.upgrades_unattended() {
            return Err(
                "unattended upgrades never run in production: set the role to staging (or demo)"
                    .to_owned(),
            );
        }
        if !(self.public_url.starts_with("http://") || self.public_url.starts_with("https://")) {
            return Err(format!(
                "public_url `{}` is not an http(s) URL",
                self.public_url
            ));
        }
        if !self.administrator.contains('@') {
            return Err(format!(
                "administrator `{}` is not an email",
                self.administrator
            ));
        }
        for name in &self.connector_secrets {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err(format!("connector secret `{name}`: use A-Z, 0-9 and _"));
            }
        }
        Ok(())
    }

    /// The connector secrets, the demo's included.
    fn connector_secrets(&self) -> Vec<String> {
        let mut names = self.connector_secrets.clone();
        if self.demo && !names.iter().any(|name| name == DEMO_CONNECTOR_SECRET) {
            names.push(DEMO_CONNECTOR_SECRET.to_owned());
        }
        names
    }
}

/// Secrets the operator gives the assistant; on a replay they are already in `secrets/`.
#[derive(Debug, Default)]
pub struct Provided {
    pub smtp_url: Option<String>,
    pub storage_access_key_id: Option<String>,
    pub storage_secret_access_key: Option<String>,
    pub explain_api_key: Option<String>,
    /// The licence file Donka issued, copied into `secrets/`.
    pub licence: Option<PathBuf>,
}

/// What happened to each file.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Written(PathBuf),
    /// It existed: left as it was (a secret, or `donka.yaml` when replayed).
    Kept(PathBuf),
}

/// Prepares a new installation in `dir` from `installation`: `donka.yaml`, then everything
/// [`apply`] writes. An existing `donka.yaml` is kept and nothing else happens: run
/// `init --config donka.yaml` to apply it.
pub fn init(
    dir: &Path,
    installation: &Installation,
    provided: &Provided,
    random: impl FnMut(&mut [u8]) -> Result<(), String>,
) -> Result<Vec<Outcome>, String> {
    let config = dir.join(CONFIG);
    if config.exists() {
        return Ok(vec![Outcome::Kept(config)]);
    }
    installation.validate()?;
    create_private_dir(dir)?;
    let mut outcomes = apply(dir, installation, provided, random)?;
    let yaml = serde_yaml_ng::to_string(installation).map_err(|err| err.to_string())?;
    write_file(
        &config,
        &format!(
            "# This installation's answers (donka-admin init). Change them here, then run\n\
             # `donka-admin init --config donka.yaml`: it writes .env, runtime.env and\n\
             # docker-compose.yml again, and keeps the secrets in secrets/.\n{yaml}"
        ),
        0o600,
    )?;
    outcomes.insert(0, Outcome::Written(config));
    Ok(outcomes)
}

/// Writes the files Docker Compose reads from `installation`, and the secrets that are missing.
/// Generated secrets are random; the others come from `provided`, or must already be there.
pub fn apply(
    dir: &Path,
    installation: &Installation,
    provided: &Provided,
    mut random: impl FnMut(&mut [u8]) -> Result<(), String>,
) -> Result<Vec<Outcome>, String> {
    installation.validate()?;
    let secrets = dir.join(SECRETS);
    create_secrets_dir(&secrets)?;
    let mut outcomes = Vec::new();
    let mut secret = |name: &str, value: &mut dyn FnMut() -> Result<String, String>| {
        let path = secrets.join(name);
        if path.exists() {
            outcomes.push(Outcome::Kept(path));
            return Ok::<_, String>(());
        }
        write_file(&path, &value()?, 0o644)?;
        outcomes.push(Outcome::Written(path));
        Ok(())
    };
    let hex = |random: &mut dyn FnMut(&mut [u8]) -> Result<(), String>| {
        let mut bytes = [0u8; 24];
        random(&mut bytes)?;
        Ok::<_, String>(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
    };
    let password_path = secrets.join(POSTGRES_PASSWORD);
    secret(POSTGRES_PASSWORD, &mut || hex(&mut random))?;
    let password = read_secret(&password_path)?;
    secret(DATABASE_URL, &mut || {
        Ok(format!("postgres://donka:{password}@postgres:5432/donka"))
    })?;
    for name in [DECISION_LOG_KEY, SECOND_STEP_KEY, PERSON_INDEX_KEY] {
        secret(name, &mut || {
            let mut bytes = [0u8; 32];
            random(&mut bytes)?;
            Ok(B64.encode(bytes))
        })?;
    }
    let operator = |value: &Option<String>, name: &str, what: &'static str| {
        from_operator(value.clone(), secrets.join(name), what)
    };
    if let Mail::Smtp { .. } = installation.mail {
        secret(
            SMTP_URL,
            &mut operator(&provided.smtp_url, SMTP_URL, "the SMTP URL"),
        )?;
    }
    if let Storage::S3 { .. } = installation.storage {
        secret(
            STORAGE_ACCESS_KEY_ID,
            &mut operator(
                &provided.storage_access_key_id,
                STORAGE_ACCESS_KEY_ID,
                "the S3 access key id",
            ),
        )?;
        secret(
            STORAGE_SECRET_ACCESS_KEY,
            &mut operator(
                &provided.storage_secret_access_key,
                STORAGE_SECRET_ACCESS_KEY,
                "the S3 secret access key",
            ),
        )?;
    }
    if installation.explanations != Explanations::Off {
        secret(
            EXPLAIN_API_KEY,
            &mut operator(
                &provided.explain_api_key,
                EXPLAIN_API_KEY,
                "the language model's API key",
            ),
        )?;
    }
    if installation.licence {
        let source = provided.licence.clone();
        secret(LICENCE, &mut || match &source {
            Some(file) => std::fs::read_to_string(file)
                .map_err(|err| format!("cannot read the licence file {}: {err}", file.display())),
            None => Err(format!(
                "{} is missing: copy Donka's licence file there",
                secrets.join(LICENCE).display()
            )),
        })?;
    }
    for name in installation.connector_secrets() {
        let file = connector_file(&name);
        let demo = installation.demo && name == DEMO_CONNECTOR_SECRET;
        let path = secrets.join(&file);
        secret(&file, &mut || {
            if demo {
                hex(&mut random)
            } else {
                Err(format!(
                    "{} is missing: write the connector secret {name} there",
                    path.display()
                ))
            }
        })?;
    }
    for (name, content) in [
        (".env", env_file(installation)),
        ("runtime.env", runtime_env(installation)),
        ("docker-compose.yml", COMPOSE.to_owned()),
    ] {
        let path = dir.join(name);
        replace_file(&path, &content)?;
        outcomes.push(Outcome::Written(path));
    }
    Ok(outcomes)
}

/// A secret the operator gives: `value` when the assistant asked for it, otherwise it must
/// already be in its file.
fn from_operator(
    value: Option<String>,
    path: PathBuf,
    what: &'static str,
) -> impl FnMut() -> Result<String, String> {
    move || match &value {
        Some(value) => Ok(value.clone()),
        None => Err(format!(
            "{} is missing: write {what} there, then run init --config {CONFIG} again",
            path.display()
        )),
    }
}

/// Records new image versions in `donka.yaml` and writes the files again (`donka-admin
/// upgrade`). Only the two version lines change, so the operator's comments stay.
pub fn set_versions(dir: &Path, studio: &str, runtime: &str) -> Result<(), String> {
    let path = dir.join(CONFIG);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let text = with_versions(&text, studio, runtime);
    replace_file(&path, &text)?;
    let installation = Installation::read(dir)?;
    apply(dir, &installation, &Provided::default(), |_| {
        Err("an upgrade generates no secret".to_owned())
    })?;
    Ok(())
}

/// Records the modules turned on in `donka.yaml` (`None`: every module the licence includes) and
/// writes the files again (`donka-admin modules`). Only the `modules` entry changes.
pub fn set_modules(dir: &Path, modules: Option<&BTreeSet<Module>>) -> Result<(), String> {
    let path = dir.join(CONFIG);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    replace_file(&path, &with_modules(&text, modules))?;
    let installation = Installation::read(dir)?;
    apply(dir, &installation, &Provided::default(), |_| {
        Err("changing modules generates no secret".to_owned())
    })?;
    Ok(())
}

/// `text` without its `modules` entry (inline or as a list), then the new one at the end.
fn with_modules(text: &str, modules: Option<&BTreeSet<Module>>) -> String {
    let mut in_modules = false;
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        if !line.starts_with(' ') && !line.starts_with('-') && !line.starts_with('#') {
            in_modules = line.starts_with("modules:");
        } else if in_modules && line.starts_with('#') {
            in_modules = false;
        }
        if !in_modules {
            lines.push(line);
        }
    }
    let mut text = lines.join("\n") + "\n";
    if let Some(modules) = modules {
        let names: Vec<&str> = modules.iter().map(|module| module.as_str()).collect();
        text.push_str(&format!("modules: [{}]\n", names.join(", ")));
    }
    text
}

fn with_versions(text: &str, studio: &str, runtime: &str) -> String {
    let mut in_images = false;
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if !line.starts_with(' ') && !line.starts_with('#') {
            in_images = line.trim_end() == "images:";
        }
        let replaced = match line.trim_start() {
            l if in_images && l.starts_with("studio:") => Some(format!("  studio: '{studio}'")),
            l if in_images && l.starts_with("runtime:") => Some(format!("  runtime: '{runtime}'")),
            _ => None,
        };
        lines.push(replaced.unwrap_or_else(|| line.to_owned()));
    }
    lines.join("\n") + "\n"
}

/// The value of a secret file, as the containers read it.
pub fn read_secret(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map(|value| value.trim_end_matches(['\n', '\r']).to_owned())
        .map_err(|err| format!("cannot read {}: {err}", path.display()))
}

fn connector_file(name: &str) -> String {
    format!("connector_{}", name.to_ascii_lowercase())
}

fn mounted(name: &str) -> String {
    format!("{MOUNTED}/{name}")
}

/// `.env`: what Compose interpolates and Studio reads. No secret, only the files holding them.
fn env_file(installation: &Installation) -> String {
    let i = installation;
    let mut lines = vec![
        "# Written by donka-admin init from donka.yaml: change donka.yaml, then run".to_owned(),
        "# `donka-admin init --config donka.yaml`. Secrets are files in secrets/.".to_owned(),
        String::new(),
        format!("DONKA_VERSION={}", i.images.studio),
        format!("DONKA_RUNTIME_VERSION={}", i.images.runtime),
    ];
    if let Some(registry) = &i.images.registry {
        lines.push(format!("DONKA_REGISTRY={registry}"));
    }
    lines.push(format!("DONKA_INSTALLATION_ROLE={}", i.role.as_str()));
    let profiles: Vec<&str> = [(i.demo, "demo"), (i.auto_upgrade.is_some(), UPGRADER)]
        .into_iter()
        .filter_map(|(on, profile)| on.then_some(profile))
        .collect();
    if !profiles.is_empty() {
        lines.push(format!("COMPOSE_PROFILES={}", profiles.join(",")));
    }
    lines.extend([
        String::new(),
        format!("DONKA_PUBLIC_URL={}", i.public_url),
        format!(
            "DONKA_COOKIE_SECURE={}",
            i.public_url.starts_with("https://")
        ),
        format!("DONKA_BOOTSTRAP_ADMIN_EMAIL={}", i.administrator),
        format!("DONKA_DEFAULT_LOCALE={}", i.language.as_str()),
        format!("DONKA_MFA_REQUIRED={}", i.second_step.as_str()),
        String::new(),
        format!("DATABASE_URL_FILE={}", mounted(DATABASE_URL)),
        format!("DONKA_DECISION_LOG_KEY_FILE={}", mounted(DECISION_LOG_KEY)),
        format!("DONKA_SECOND_STEP_KEY_FILE={}", mounted(SECOND_STEP_KEY)),
        format!("DONKA_PERSON_INDEX_KEY_FILE={}", mounted(PERSON_INDEX_KEY)),
    ]);
    match &i.mail {
        Mail::Mailpit => lines.extend([
            "DONKA_SMTP_URL=smtp://mailpit:1025".to_owned(),
            "DONKA_SMTP_FROM=Donka <donka@localhost>".to_owned(),
        ]),
        Mail::Smtp { from } => lines.extend([
            format!("DONKA_SMTP_URL_FILE={}", mounted(SMTP_URL)),
            format!("DONKA_SMTP_FROM={from}"),
        ]),
    }
    match &i.storage {
        Storage::Disk => lines.push("DONKA_STORAGE_URL=file:///var/lib/donka/releases".to_owned()),
        Storage::S3 {
            bucket,
            region,
            endpoint,
        } => {
            lines.extend([
                format!("DONKA_STORAGE_URL=s3://{bucket}"),
                format!("DONKA_STORAGE_REGION={region}"),
                format!(
                    "DONKA_STORAGE_ACCESS_KEY_ID_FILE={}",
                    mounted(STORAGE_ACCESS_KEY_ID)
                ),
                format!(
                    "DONKA_STORAGE_SECRET_ACCESS_KEY_FILE={}",
                    mounted(STORAGE_SECRET_ACCESS_KEY)
                ),
            ]);
            if let Some(endpoint) = endpoint {
                lines.push(format!("DONKA_STORAGE_ENDPOINT={endpoint}"));
            }
        }
    }
    let explain = |api: &str, url: &str, model: &str| {
        [
            format!("DONKA_EXPLAIN_API={api}"),
            format!("DONKA_EXPLAIN_URL={url}"),
            format!("DONKA_EXPLAIN_MODEL={model}"),
            format!("DONKA_EXPLAIN_API_KEY_FILE={}", mounted(EXPLAIN_API_KEY)),
        ]
    };
    match &i.explanations {
        Explanations::Off => {}
        Explanations::Anthropic { url, model } => lines.extend(explain("anthropic", url, model)),
        Explanations::Openai { url, model } => lines.extend(explain("openai", url, model)),
    }
    if i.licence {
        lines.push(format!("DONKA_LICENCE_FILE={}", mounted(LICENCE)));
    }
    if i.demo {
        lines.push("DONKA_DEMO=true".to_owned());
    }
    if i.offline {
        lines.push("DONKA_UPDATE_CHECK=false".to_owned());
    }
    if let Some(window) = &i.auto_upgrade {
        lines.push(format!("DONKA_AUTO_UPGRADE_WINDOW={window}"));
    }
    if let Some(modules) = &i.modules {
        let names: Vec<&str> = modules.iter().map(|module| module.as_str()).collect();
        lines.push(format!("DONKA_MODULES={}", names.join(",")));
    }
    lines.join("\n") + "\n"
}

/// `runtime.env`: the staging Runtime's settings, and the files holding its secrets.
fn runtime_env(installation: &Installation) -> String {
    let mut lines = vec![
        "# Written by donka-admin init from donka.yaml: the staging Runtime's settings.".to_owned(),
        "DECISION_LOG__URL=http://app:8080/api/v1/decision-log/records".to_owned(),
        "DECISION_LOG__TOKEN_FILE=/var/lib/donka/wiring/staging-decision-log-token".to_owned(),
    ];
    match &installation.storage {
        Storage::Disk => lines.extend([
            "PROVIDER__TYPE=Zip".to_owned(),
            "PROVIDER__ROOT_DIR=/var/lib/donka/releases/staging".to_owned(),
            "PROVIDER__WATCH=true".to_owned(),
        ]),
        Storage::S3 {
            bucket,
            region,
            endpoint,
        } => {
            lines.extend([
                "PROVIDER__TYPE=S3".to_owned(),
                format!("PROVIDER__BUCKET={bucket}"),
                format!("PROVIDER__REGION={region}"),
                "PROVIDER__PREFIX=staging/".to_owned(),
                format!("AWS_ACCESS_KEY_ID_FILE={}", mounted(STORAGE_ACCESS_KEY_ID)),
                format!(
                    "AWS_SECRET_ACCESS_KEY_FILE={}",
                    mounted(STORAGE_SECRET_ACCESS_KEY)
                ),
            ]);
            if let Some(endpoint) = endpoint {
                lines.extend([
                    format!("PROVIDER__ENDPOINT={endpoint}"),
                    "PROVIDER__FORCE_PATH_STYLE=true".to_owned(),
                ]);
            }
        }
    }
    for name in installation.connector_secrets() {
        lines.push(format!(
            "DONKA_SECRET_{name}_FILE={}",
            mounted(&connector_file(&name))
        ));
    }
    lines.join("\n") + "\n"
}

/// The installation folder holds the secrets: only its owner may enter it.
fn create_private_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    set_mode(dir, 0o700)
}

/// Containers run as other users: they may read a secret by name, but not list the folder.
/// The installation folder around it keeps everyone else out.
fn create_secrets_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    set_mode(dir, 0o711)
}

fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|err| format!("cannot protect {}: {err}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Creates `path`, which must not exist yet.
fn write_file(path: &Path, content: &str, mode: u32) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = options
        .open(path)
        .map_err(|err| format!("cannot create {}: {err}", path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// Writes `path` whole, through a temporary file, readable by its owner only. A file that
/// existed keeps its owner, so the operator keeps their files when the auto-upgrade service,
/// which runs as root, writes them.
fn replace_file(path: &Path, content: &str) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&temp);
    write_file(&temp, content, 0o600)?;
    #[cfg(unix)]
    if let Ok(old) = std::fs::metadata(path) {
        use std::os::unix::fs::MetadataExt as _;
        // Only root may give a file away; anyone else already owns what they write.
        let _ = std::os::unix::fs::chown(&temp, Some(old.uid()), Some(old.gid()));
    }
    std::fs::rename(&temp, path).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

#[cfg(test)]
mod tests;
