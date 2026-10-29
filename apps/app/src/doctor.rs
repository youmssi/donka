//! `donka-admin doctor` (DNK-74): checks what Studio depends on with Studio's own settings, and
//! says for each what works, or why not and which setting to change. It changes nothing but a
//! test file in the release storage, removed at once, and sends no email and no decision.
//!
//! Run it where Studio runs, so that it reaches what Studio reaches:
//! `docker compose run --rm --no-deps app donka-admin doctor`.

use crate::config::{Config, MfaRequired};
use donka_db::DbOptions;
use donka_licence::{PublicKey, Status};
use donka_mail::{MailError, SmtpMailer};
use donka_storage::ObjectStorage;
use reqwest::Url;
use std::collections::BTreeMap;
use std::future::Future;
use std::time::Duration;

/// Time allowed to each check.
const TIMEOUT: Duration = Duration::from_secs(20);
/// Time allowed to reach each connector endpoint.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Working(String),
    /// Why, in plain words, and what to change.
    Failing {
        reason: String,
        fix: String,
    },
    /// Not configured, so nothing to reach.
    Off(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub outcome: Outcome,
}

impl Check {
    fn working(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            outcome: Outcome::Working(detail.into()),
        }
    }

    fn failing(name: &'static str, reason: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            outcome: Outcome::Failing {
                reason: reason.into(),
                fix: fix.into(),
            },
        }
    }

    fn off(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            outcome: Outcome::Off(detail.into()),
        }
    }

    pub fn failed(&self) -> bool {
        matches!(self.outcome, Outcome::Failing { .. })
    }
}

/// Reads Studio's settings, then checks each dependency. Settings Studio refuses stop here:
/// Studio would not start either.
pub async fn run(
    settings: Result<Config, crate::config::ConfigError>,
    key: PublicKey,
) -> Vec<Check> {
    let config = match settings {
        Ok(config) => config,
        Err(err) => {
            return vec![Check::failing(
                "settings",
                format!("{} {}", err.var, err.reason),
                format!(
                    "check {}: in donka.yaml or secrets/, then donka-admin init --config \
                     donka.yaml, on an installation init prepared; in Studio's environment \
                     otherwise",
                    err.var
                ),
            )]
        }
    };
    let mut checks = vec![Check::working("settings", "Studio accepts them")];
    let pool = database(&config, &mut checks).await;
    checks.push(timed("storage", storage(&config)).await);
    checks.push(timed("mail", mail(&config)).await);
    checks.push(sign_in(config.mfa_required));
    checks.push(timed("language model", language_model(&config)).await);
    checks.push(licence(&config, &key));
    checks.push(packs(&config));
    if let Some(pool) = pool {
        checks.extend(connectors(&pool).await);
    }
    checks
}

/// The report, one line per check and the fix under a failing one.
pub fn render(checks: &[Check]) -> String {
    let width = checks
        .iter()
        .map(|check| check.name.len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for check in checks {
        let (mark, text) = match &check.outcome {
            Outcome::Working(detail) => ("ok  ", detail.as_str()),
            Outcome::Off(detail) => ("off ", detail.as_str()),
            Outcome::Failing { reason, .. } => ("FAIL", reason.as_str()),
        };
        out.push_str(&format!("  {mark}  {:width$}  {text}\n", check.name));
        if let Outcome::Failing { fix, .. } = &check.outcome {
            out.push_str(&format!("        {:width$}  → {fix}\n", ""));
        }
    }
    let failing = checks.iter().filter(|check| check.failed()).count();
    out.push_str(&match failing {
        0 => "doctor: everything Studio depends on works\n".to_owned(),
        1 => "doctor: 1 check failed\n".to_owned(),
        n => format!("doctor: {n} checks failed\n"),
    });
    out
}

async fn timed(name: &'static str, check: impl Future<Output = Check>) -> Check {
    tokio::time::timeout(TIMEOUT, check)
        .await
        .unwrap_or_else(|_| {
            Check::failing(
                name,
                format!("no answer within {} seconds", TIMEOUT.as_secs()),
                "check the address and that a firewall lets Studio reach it",
            )
        })
}

/// Reaches PostgreSQL and counts the migrations not applied yet.
async fn database(config: &Config, checks: &mut Vec<Check>) -> Option<donka_db::PgPool> {
    const NAME: &str = "database";
    let fix = "check DATABASE_URL (secrets/database_url) and that PostgreSQL is running";
    // One plain connection first: a pool would retry until its timeout and hide why.
    // The URL holds the password: the error names the problem, never the URL.
    let connected = tokio::time::timeout(
        TIMEOUT,
        <sqlx::PgConnection as sqlx::Connection>::connect(&config.database_url),
    )
    .await
    .unwrap_or(Err(sqlx::Error::PoolTimedOut));
    if let Err(err) = connected {
        checks.push(Check::failing(NAME, plain_database_error(&err), fix));
        return None;
    }
    let options = DbOptions {
        max_connections: 2,
        acquire_timeout: TIMEOUT,
    };
    // The URL was just used to connect: it parses.
    let pool = donka_db::connect_lazy(&config.database_url, &options).ok()?;
    checks.push(match donka_db::pending_migrations(&pool).await {
        Ok(0) => Check::working(NAME, "reachable, every migration applied"),
        Ok(n) if config.db_migrate => Check::working(
            NAME,
            format!("reachable; Studio applies {n} pending migrations at its next start"),
        ),
        Ok(n) => Check::failing(
            NAME,
            format!("{n} migrations are not applied, and Studio does not apply them"),
            "set DONKA_DB_MIGRATE=true, or apply them as your database team does",
        ),
        Err(err) => Check::failing(NAME, plain_database_error(&err), fix),
    });
    Some(pool)
}

fn plain_database_error(err: &sqlx::Error) -> String {
    match err {
        sqlx::Error::Io(err) => format!("cannot reach PostgreSQL: {err}"),
        sqlx::Error::Tls(err) => format!("the TLS connection to PostgreSQL failed: {err}"),
        sqlx::Error::PoolTimedOut => "PostgreSQL did not answer in time".to_owned(),
        sqlx::Error::Database(err) => format!("PostgreSQL refused: {}", err.message()),
        sqlx::Error::Configuration(_) => {
            "DATABASE_URL is not a PostgreSQL URL (postgres://user:password@host:5432/db)"
                .to_owned()
        }
        other => other.to_string(),
    }
}

/// Writes and removes a test file where releases go.
async fn storage(config: &Config) -> Check {
    const NAME: &str = "storage";
    let fix = if config.storage_url.starts_with("s3://") {
        "check DONKA_STORAGE_URL, DONKA_STORAGE_REGION and DONKA_STORAGE_ENDPOINT, and the keys \
         in secrets/storage_access_key_id and secrets/storage_secret_access_key: they must allow \
         writing and deleting in the bucket"
    } else {
        "check DONKA_STORAGE_URL, and that the folder exists and Studio may write in it"
    };
    let store = match ObjectStorage::from_url(&config.storage_url, config.storage_options.clone()) {
        Ok(store) => store,
        Err(err) => return Check::failing(NAME, err.to_string(), fix),
    };
    match store.check().await {
        Ok(()) => Check::working(NAME, format!("{} is writable", config.storage_url)),
        Err(err) => Check::failing(NAME, err.to_string(), fix),
    }
}

/// Connects to the mail server and signs in, without sending anything.
async fn mail(config: &Config) -> Check {
    const NAME: &str = "mail";
    let fix = "check the SMTP URL (secrets/smtp_url, DONKA_SMTP_URL): host, port, user, \
               password and tls=required for STARTTLS or smtps:// for implicit TLS";
    let mailer = match SmtpMailer::new(&config.smtp_url, &config.smtp_from) {
        Ok(mailer) => mailer,
        Err(err) => {
            return Check::failing(
                NAME,
                err.to_string(),
                "check DONKA_SMTP_URL and DONKA_SMTP_FROM (Donka <donka@bank.example>)",
            )
        }
    };
    match mailer.check().await {
        Ok(()) => Check::working(NAME, "the server accepts Studio's connection"),
        Err(MailError::Rejected(reason)) => {
            Check::failing(NAME, format!("the server refused: {reason}"), fix)
        }
        Err(MailError::Send(reason)) => {
            Check::failing(NAME, format!("cannot talk to the server: {reason}"), fix)
        }
        Err(err) => Check::failing(NAME, err.to_string(), fix),
    }
}

/// Local accounts until single sign-on exists (DNK-60): nothing to reach.
fn sign_in(required: MfaRequired) -> Check {
    let who = match required {
        MfaRequired::None => "a second step for whoever turns one on",
        MfaRequired::Privileged => "a second step for administrators and project owners",
        MfaRequired::All => "a second step for everyone",
    };
    Check::working("sign-in", format!("local accounts, {who}"))
}

/// Asks the model for one word: the endpoint, the key and the model all work.
async fn language_model(config: &Config) -> Check {
    const NAME: &str = "language model";
    let Some(settings) = config.explain.clone() else {
        return Check::off(NAME, "explanations are off");
    };
    let model = settings.model.clone();
    let url = settings.url.clone();
    let explainer = match donka_explain::Explainer::new(settings) {
        Ok(explainer) => explainer,
        Err(err) => return Check::failing(NAME, err, "check DONKA_EXPLAIN_URL"),
    };
    match explainer.check().await {
        Ok(answer) => Check::working(NAME, format!("{} answers at {url}", answer.model)),
        Err(donka_explain::ExplainError::Declined) => {
            Check::working(NAME, format!("{model} answers at {url}"))
        }
        Err(donka_explain::ExplainError::Status(401 | 403)) => Check::failing(
            NAME,
            format!("{url} refused the key"),
            "check the key in secrets/explain_api_key (DONKA_EXPLAIN_API_KEY)",
        ),
        Err(donka_explain::ExplainError::Status(code @ (400 | 404))) => Check::failing(
            NAME,
            format!("{url} answered HTTP {code}: the address or the model is not one it knows"),
            "check DONKA_EXPLAIN_URL, DONKA_EXPLAIN_API and DONKA_EXPLAIN_MODEL",
        ),
        Err(err) => Check::failing(
            NAME,
            format!("{err} ({url})"),
            "check DONKA_EXPLAIN_URL and that a firewall lets Studio reach it",
        ),
    }
}

fn licence(config: &Config, key: &PublicKey) -> Check {
    const NAME: &str = "licence";
    let Some(path) = &config.licence_file else {
        return Check::off(NAME, "none: evaluation, releases cannot reach production");
    };
    let verified = std::fs::read_to_string(path)
        .map_err(|err| {
            donka_licence::LicenceError::Unreadable(format!(
                "cannot read {}: {err}",
                path.display()
            ))
        })
        .and_then(|text| key.verify(&text));
    let fix = "install the licence file Donka sent for this installation (secrets/licence.json)";
    match Status::on(Some(verified), chrono::Utc::now().date_naive()) {
        Status::Active { licence, days_left } => Check::working(
            NAME,
            format!(
                "{}, valid until {} ({days_left} days left)",
                licence.customer, licence.valid_until
            ),
        ),
        Status::NotYetValid { licence } => {
            Check::failing(NAME, format!("valid from {} only", licence.valid_from), fix)
        }
        Status::Expired { licence } => Check::failing(
            NAME,
            format!("ended on {}", licence.valid_until),
            "ask Donka for a new licence, then install it (secrets/licence.json)",
        ),
        Status::Refused { reason } => Check::failing(NAME, reason.to_string(), fix),
        Status::Evaluation => Check::off(NAME, "none: evaluation"),
    }
}

fn packs(config: &Config) -> Check {
    const NAME: &str = "packs";
    let Some(dir) = &config.packs_dir else {
        return Check::off(NAME, "none offered (DONKA_PACKS_DIR is not set)");
    };
    match donka_pack::Catalogue::load(dir) {
        Ok(catalogue) => Check::working(NAME, format!("{} offered", catalogue.packs().len())),
        Err(err) => Check::failing(
            NAME,
            err.to_string(),
            "fix or remove that pack, or point DONKA_PACKS_DIR at another folder",
        ),
    }
}

/// Reaches each service the live decisions' connectors call: its name resolves and its port
/// accepts a connection. Nothing is sent: TLS, credentials and answers are the Runtime's to
/// check at each call. From Studio's network, like the staging Runtime's; a production
/// Runtime on another network may see them differently.
pub async fn connectors(pool: &donka_db::PgPool) -> Vec<Check> {
    const NAME: &str = "connectors";
    let decisions = match donka_release::live_decisions(pool).await {
        Ok(decisions) => decisions,
        Err(err) => {
            return vec![Check::failing(
                NAME,
                plain_database_error(&err),
                "check the database first",
            )]
        }
    };
    // Each address once, with the decisions that call it.
    let mut endpoints: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for decision in &decisions {
        for url in donka_engine::connector_endpoints(&decision.content) {
            endpoints.entry(url).or_default().push(format!(
                "{}/{} on {}",
                decision.project_key,
                decision.key,
                decision.environment.as_str()
            ));
        }
    }
    if endpoints.is_empty() {
        return vec![Check::off(NAME, "no live decision calls a service")];
    }
    let mut checks = Vec::with_capacity(endpoints.len());
    for (url, users) in endpoints {
        let outcome = match reach(&url).await {
            Ok(()) => Outcome::Working(format!("{url} is reachable")),
            Err(reason) => Outcome::Failing {
                reason: format!("{url}: {reason}"),
                fix: format!(
                    "check the address in the connector ({}) and that a firewall lets the \
                     Runtime reach it",
                    users.join(", ")
                ),
            },
        };
        checks.push(Check {
            name: NAME,
            outcome,
        });
    }
    checks
}

/// The host of `url` resolves and its port accepts a TCP connection.
pub async fn reach(url: &str) -> Result<(), String> {
    let url = Url::parse(url).map_err(|err| format!("not an address: {err}"))?;
    let host = url.host_str().ok_or("the address names no host")?;
    let port = url
        .port_or_known_default()
        .ok_or("the address names no port")?;
    let addresses: Vec<_> = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio::net::lookup_host((host.trim_matches(['[', ']']), port)),
    )
    .await
    .map_err(|_| format!("{host} did not resolve in time"))?
    .map_err(|_| format!("{host} does not resolve: check the name and the DNS"))?
    .collect();
    let mut last = format!("{host} has no address");
    for address in addresses {
        match tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(address)).await {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(err)) => last = format!("{host}:{port} refused the connection ({err})"),
            Err(_) => {
                last = format!(
                    "{host}:{port} did not answer within {} seconds",
                    CONNECT_TIMEOUT.as_secs()
                )
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests;
