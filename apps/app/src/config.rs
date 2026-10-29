use donka_identity::Locale;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Settings read from the environment at startup (documented in `.env.example`).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub listen: SocketAddr,
    pub engine_workers: Option<usize>,
    /// Versioned prefix every endpoint lives under, e.g. `/api/v1`.
    pub api_base_path: String,
    /// PostgreSQL connection string. Contains credentials: never log it.
    pub database_url: String,
    /// Apply pending migrations at startup.
    pub db_migrate: bool,
    pub db_max_connections: u32,
    /// Where people reach Studio, used to build links (e.g. password setup).
    pub public_url: String,
    /// Send the session cookie only over HTTPS. Turn off only for local HTTP.
    pub cookie_secure: bool,
    /// Export traces and request metrics over OTLP (`DONKA_OTEL_ENABLED`); off by default.
    pub otel_enabled: bool,
    pub session_idle_minutes: u32,
    pub sign_in_max_failures: u32,
    pub sign_in_lock_minutes: u32,
    /// Sign-in attempts per minute from one client address (DNK-32).
    pub sign_in_rate_limit: u32,
    /// Password-reset requests per minute from one client address.
    pub password_reset_rate_limit: u32,
    /// Reverse proxies whose `X-Forwarded-For` names the client (addresses or CIDR ranges).
    pub trusted_proxies: Vec<ipnet::IpNet>,
    /// On an empty database, create this administrator and print a setup link.
    pub bootstrap_admin_email: Option<String>,
    /// Language of the first administrator (others choose theirs when invited).
    pub default_locale: Locale,
    pub invitation_link_hours: u32,
    pub password_reset_link_minutes: u32,
    /// SMTP server, e.g. `smtp://user:pass@host:587?tls=required`. Contains
    /// credentials: never log it.
    pub smtp_url: String,
    /// Sender of account emails, e.g. `Donka <donka@bank.example>`.
    pub smtp_from: String,
    /// Failed sends after which an account email is abandoned.
    pub email_max_attempts: u32,
    /// Static export of the web app to serve on the same origin; API only when unset.
    pub web_dir: Option<PathBuf>,
    /// Where release artifacts are written: `s3://bucket[/prefix]` (S3, MinIO)
    /// or `file:///path` (a folder, e.g. on a single server).
    pub storage_url: String,
    /// Settings of an S3 store as `object_store` names them (`aws_endpoint`,
    /// `aws_region`, keys…). The secret key is a credential: never log them.
    pub storage_options: Vec<(String, String)>,
    /// Failed writes after which a deployment is given up (it can be retried by hand).
    pub publish_max_attempts: u32,
    /// Base64 of the 32-byte key decision records are encrypted with. A secret:
    /// never log it, and keep it safe (records cannot be read without it).
    pub decision_log_key: String,
    /// Keys records were sealed with before a rotation (DNK-40): they open those
    /// records until `donka-app decision-log reseal` re-seals them, and never
    /// seal. Secrets, like the current key.
    pub decision_log_previous_keys: Vec<String>,
    /// Base64 of the 32-byte key the identifiers people are found by are hashed
    /// with (DNK-75). A secret; changing it means indexing the log again.
    pub person_index_key: String,
    /// Base64 of the 32-byte key second-step secrets are sealed with (DNK-59). A
    /// secret: without it, nobody with a second step can sign in.
    pub second_step_key: String,
    /// Keys second-step secrets were sealed with before a rotation: they still
    /// open them, and each secret is re-sealed with the current key at its next use.
    pub second_step_previous_keys: Vec<String>,
    /// Who must use a second sign-in step (`DONKA_MFA_REQUIRED`).
    pub mfa_required: MfaRequired,
    /// Decision records older than this are purged.
    pub decision_log_retention_days: u32,
    /// The customer's LLM endpoint that explains logged decisions; `None`
    /// (the default) turns explanations off. Holds a key: never log it.
    pub explain: Option<donka_explain::Settings>,
    /// Folder of the packs offered when a project is created (DNK-43), one folder per
    /// pack; none are offered when unset. Projects can still be made from a pack file.
    pub packs_dir: Option<PathBuf>,
    /// The licence file Donka issued for this installation (DNK-77). Unset: evaluation, where
    /// everything works but nothing reaches production.
    pub licence_file: Option<PathBuf>,
    /// Where Studio writes, at first start, the decision-log token of the staging Runtime it
    /// is installed with (DNK-64). Unset: Runtimes get their token from an administrator.
    pub runtime_log_token_file: Option<PathBuf>,
    /// Set by `donka-admin init --demo` (DNK-64): at the first start, the retail-credit pack is
    /// imported, released and deployed to staging as the first administrator.
    pub demo: bool,
}

/// Who must sign in with a second step (DNK-59). Anyone may turn one on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaRequired {
    None,
    /// Instance administrators and anyone owning a project.
    Privileged,
    All,
}

/// Settings that hold a secret, each with the setting naming a file that holds it instead
/// (`X_FILE`): Docker and Kubernetes secrets, and vault agents, deliver secrets as files.
pub const SECRET_SETTINGS: [(&str, &str); 10] = [
    ("DATABASE_URL", "DATABASE_URL_FILE"),
    ("DONKA_SMTP_URL", "DONKA_SMTP_URL_FILE"),
    (
        "DONKA_STORAGE_ACCESS_KEY_ID",
        "DONKA_STORAGE_ACCESS_KEY_ID_FILE",
    ),
    (
        "DONKA_STORAGE_SECRET_ACCESS_KEY",
        "DONKA_STORAGE_SECRET_ACCESS_KEY_FILE",
    ),
    ("DONKA_DECISION_LOG_KEY", "DONKA_DECISION_LOG_KEY_FILE"),
    (
        "DONKA_DECISION_LOG_PREVIOUS_KEYS",
        "DONKA_DECISION_LOG_PREVIOUS_KEYS_FILE",
    ),
    ("DONKA_PERSON_INDEX_KEY", "DONKA_PERSON_INDEX_KEY_FILE"),
    ("DONKA_SECOND_STEP_KEY", "DONKA_SECOND_STEP_KEY_FILE"),
    (
        "DONKA_SECOND_STEP_PREVIOUS_KEYS",
        "DONKA_SECOND_STEP_PREVIOUS_KEYS_FILE",
    ),
    ("DONKA_EXPLAIN_API_KEY", "DONKA_EXPLAIN_API_KEY_FILE"),
];

/// The web app calls the API here (apps/web `API_BASE`).
const WEB_API_BASE_PATH: &str = "/api/v1";

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{var} {reason}")]
pub struct ConfigError {
    pub var: &'static str,
    pub reason: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads settings through `get` so tests do not touch the process environment. A secret
    /// setting given as a file (`X_FILE`) is read first, as if it were `X`.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let files = secret_files(&get)?;
        let get = |key: &str| files.get(key).cloned().or_else(|| get(key));
        let listen = get("DONKA_LISTEN")
            .unwrap_or_else(|| "0.0.0.0:8080".into())
            .parse()
            .map_err(|_| invalid("DONKA_LISTEN", "must be host:port, e.g. 0.0.0.0:8080"))?;

        let api_base_path = get("DONKA_API_BASE_PATH").unwrap_or_else(|| "/api/v1".into());
        validate_base_path(&api_base_path)?;

        let database_url = required(
            &get,
            "DATABASE_URL",
            "is required, e.g. postgres://user:pass@host:5432/donka",
        )?;
        let smtp_url = required(
            &get,
            "DONKA_SMTP_URL",
            "is required, e.g. smtp://user:pass@mail.bank.example:587?tls=required",
        )?;
        let smtp_from = required(
            &get,
            "DONKA_SMTP_FROM",
            "is required, e.g. Donka <donka@bank.example>",
        )?;
        let default_locale = match get("DONKA_DEFAULT_LOCALE") {
            None => Locale::En,
            Some(value) => value
                .parse()
                .map_err(|reason: String| invalid("DONKA_DEFAULT_LOCALE", &reason))?,
        };

        let public_url = get("DONKA_PUBLIC_URL").unwrap_or_else(|| "http://localhost:8080".into());
        if !(public_url.starts_with("http://") || public_url.starts_with("https://"))
            || public_url.ends_with('/')
        {
            return Err(invalid(
                "DONKA_PUBLIC_URL",
                "must start with http:// or https:// and not end with '/'",
            ));
        }

        let web_dir = get("DONKA_WEB_DIR")
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from);
        if web_dir.is_some() && api_base_path != WEB_API_BASE_PATH {
            return Err(invalid(
                "DONKA_API_BASE_PATH",
                "must stay /api/v1 when DONKA_WEB_DIR is set: the web app calls /api/v1",
            ));
        }

        let storage_url = required(
            &get,
            "DONKA_STORAGE_URL",
            "is required, e.g. s3://donka-releases or file:///var/lib/donka/releases",
        )?;
        if !(storage_url.starts_with("s3://") || storage_url.starts_with("file:///")) {
            return Err(invalid(
                "DONKA_STORAGE_URL",
                "must start with s3:// (S3, MinIO) or file:/// (a folder)",
            ));
        }
        let mut storage_options = Vec::new();
        for (var, option) in [
            ("DONKA_STORAGE_ENDPOINT", "aws_endpoint"),
            ("DONKA_STORAGE_REGION", "aws_region"),
            ("DONKA_STORAGE_ACCESS_KEY_ID", "aws_access_key_id"),
            ("DONKA_STORAGE_SECRET_ACCESS_KEY", "aws_secret_access_key"),
        ] {
            if let Some(value) = get(var).filter(|v| !v.trim().is_empty()) {
                if option == "aws_endpoint" && value.starts_with("http://") {
                    // A local MinIO usually listens on plain HTTP.
                    storage_options.push(("aws_allow_http".to_owned(), "true".to_owned()));
                }
                storage_options.push((option.to_owned(), value));
            }
        }

        let decision_log_key = required(
            &get,
            "DONKA_DECISION_LOG_KEY",
            "is required: 32 random bytes in base64 (e.g. `openssl rand -base64 32`)",
        )?;
        donka_shared::cipher::Cipher::from_base64(&decision_log_key)
            .map_err(|err| invalid("DONKA_DECISION_LOG_KEY", &err.to_string()))?;
        let decision_log_previous_keys = previous_keys(&get, "DONKA_DECISION_LOG_PREVIOUS_KEYS")?;
        let person_index_key = required(
            &get,
            "DONKA_PERSON_INDEX_KEY",
            "is required: 32 random bytes in base64 (e.g. `openssl rand -base64 32`)",
        )?;
        donka_decision_log::PersonKey::from_base64(&person_index_key)
            .map_err(|err| invalid("DONKA_PERSON_INDEX_KEY", &err.to_string()))?;

        let second_step_key = required(
            &get,
            "DONKA_SECOND_STEP_KEY",
            "is required: 32 random bytes in base64 (e.g. `openssl rand -base64 32`)",
        )?;
        donka_shared::cipher::Cipher::from_base64(&second_step_key)
            .map_err(|err| invalid("DONKA_SECOND_STEP_KEY", &err.to_string()))?;
        let second_step_previous_keys = previous_keys(&get, "DONKA_SECOND_STEP_PREVIOUS_KEYS")?;
        let mfa_required = match get("DONKA_MFA_REQUIRED").as_deref().map(str::trim) {
            None | Some("") | Some("none") => MfaRequired::None,
            Some("privileged") => MfaRequired::Privileged,
            Some("all") => MfaRequired::All,
            Some(_) => {
                return Err(invalid(
                    "DONKA_MFA_REQUIRED",
                    "must be none, privileged or all",
                ))
            }
        };

        let explain = explain_settings(&get)?;

        Ok(Self {
            listen,
            engine_workers: positive(&get, "DONKA_ENGINE_WORKERS")?,
            api_base_path,
            database_url,
            db_migrate: boolean(&get, "DONKA_DB_MIGRATE", true)?,
            db_max_connections: positive(&get, "DONKA_DB_MAX_CONNECTIONS")?.unwrap_or(10),
            public_url,
            cookie_secure: boolean(&get, "DONKA_COOKIE_SECURE", true)?,
            otel_enabled: boolean(&get, "DONKA_OTEL_ENABLED", false)?,
            session_idle_minutes: positive(&get, "DONKA_SESSION_IDLE_MINUTES")?.unwrap_or(480),
            sign_in_max_failures: positive(&get, "DONKA_SIGN_IN_MAX_FAILURES")?.unwrap_or(5),
            sign_in_lock_minutes: positive(&get, "DONKA_SIGN_IN_LOCK_MINUTES")?.unwrap_or(15),
            sign_in_rate_limit: positive(&get, "DONKA_SIGN_IN_RATE_LIMIT")?.unwrap_or(10),
            password_reset_rate_limit: positive(&get, "DONKA_PASSWORD_RESET_RATE_LIMIT")?
                .unwrap_or(5),
            trusted_proxies: trusted_proxies(&get)?,
            bootstrap_admin_email: get("DONKA_BOOTSTRAP_ADMIN_EMAIL")
                .filter(|v| !v.trim().is_empty()),
            default_locale,
            invitation_link_hours: positive(&get, "DONKA_INVITATION_LINK_HOURS")?.unwrap_or(72),
            password_reset_link_minutes: positive(&get, "DONKA_PASSWORD_RESET_LINK_MINUTES")?
                .unwrap_or(30),
            smtp_url,
            smtp_from,
            email_max_attempts: positive(&get, "DONKA_EMAIL_MAX_ATTEMPTS")?.unwrap_or(10),
            web_dir,
            storage_url,
            storage_options,
            publish_max_attempts: positive(&get, "DONKA_PUBLISH_MAX_ATTEMPTS")?.unwrap_or(10),
            decision_log_key,
            decision_log_previous_keys,
            person_index_key,
            second_step_key,
            second_step_previous_keys,
            mfa_required,
            // Five years, a common minimum for credit files.
            decision_log_retention_days: positive(&get, "DONKA_DECISION_LOG_RETENTION_DAYS")?
                .unwrap_or(1825),
            explain,
            packs_dir: get("DONKA_PACKS_DIR")
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from),
            licence_file: get("DONKA_LICENCE_FILE")
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from),
            runtime_log_token_file: get("DONKA_RUNTIME_LOG_TOKEN_FILE")
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from),
            demo: boolean(&get, "DONKA_DEMO", false)?,
        })
    }
}

/// Keys used before a rotation, separated by commas, spaces or new lines; each
/// must be a valid key.
fn previous_keys(
    get: &impl Fn(&str) -> Option<String>,
    var: &'static str,
) -> Result<Vec<String>, ConfigError> {
    let keys: Vec<String> = get(var)
        .unwrap_or_default()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect();
    for key in &keys {
        donka_shared::cipher::Cipher::from_base64(key)
            .map_err(|err| invalid(var, &err.to_string()))?;
    }
    Ok(keys)
}

/// Explanations are on only when an endpoint is configured; then its key,
/// request format and model are required too.
fn explain_settings(
    get: &impl Fn(&str) -> Option<String>,
) -> Result<Option<donka_explain::Settings>, ConfigError> {
    let Some(url) = get("DONKA_EXPLAIN_URL").filter(|v| !v.trim().is_empty()) else {
        if get("DONKA_EXPLAIN_API_KEY").is_some_and(|v| !v.trim().is_empty()) {
            return Err(invalid(
                "DONKA_EXPLAIN_URL",
                "is required when DONKA_EXPLAIN_API_KEY is set (explanations are off without it)",
            ));
        }
        return Ok(None);
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(invalid(
            "DONKA_EXPLAIN_URL",
            "must start with https:// or http://, e.g. https://api.anthropic.com/v1/messages",
        ));
    }
    let api_key = required(
        get,
        "DONKA_EXPLAIN_API_KEY",
        "is required when DONKA_EXPLAIN_URL is set",
    )?;
    let protocol = required(
        get,
        "DONKA_EXPLAIN_API",
        "is required when DONKA_EXPLAIN_URL is set: anthropic or openai",
    )?
    .parse()
    .map_err(|reason: String| invalid("DONKA_EXPLAIN_API", &reason))?;
    let model = required(
        get,
        "DONKA_EXPLAIN_MODEL",
        "is required when DONKA_EXPLAIN_URL is set: the model name the endpoint expects",
    )?;
    let fallbacks = boolean(get, "DONKA_EXPLAIN_FALLBACKS", false)?;
    if fallbacks && protocol != donka_explain::Protocol::Anthropic {
        return Err(invalid(
            "DONKA_EXPLAIN_FALLBACKS",
            "applies only when DONKA_EXPLAIN_API is anthropic",
        ));
    }
    Ok(Some(donka_explain::Settings {
        url,
        api_key,
        protocol,
        model,
        timeout: std::time::Duration::from_secs(
            positive(get, "DONKA_EXPLAIN_TIMEOUT_SECONDS")?.unwrap_or(60),
        ),
        fallbacks,
    }))
}

/// The secrets given as files, by setting name: the file's content without trailing newlines.
/// Setting both forms, or naming a file that cannot be read, fails and names the setting.
fn secret_files(
    get: &impl Fn(&str) -> Option<String>,
) -> Result<std::collections::HashMap<&'static str, String>, ConfigError> {
    let mut files = std::collections::HashMap::new();
    for (var, file_var) in SECRET_SETTINGS {
        let Some(path) = get(file_var).filter(|v| !v.trim().is_empty()) else {
            continue;
        };
        if get(var).is_some_and(|v| !v.trim().is_empty()) {
            return Err(invalid(
                var,
                &format!("and {file_var} are both set: keep one"),
            ));
        }
        let value = std::fs::read_to_string(path.trim())
            .map_err(|err| invalid(file_var, &format!("cannot be read: {err}")))?;
        files.insert(var, value.trim_end_matches(['\n', '\r']).to_owned());
    }
    Ok(files)
}

fn required(
    get: &impl Fn(&str) -> Option<String>,
    var: &'static str,
    reason: &str,
) -> Result<String, ConfigError> {
    get(var)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| invalid(var, reason))
}

/// An optional variable that must be a number greater than zero when set.
fn positive<T>(
    get: &impl Fn(&str) -> Option<String>,
    var: &'static str,
) -> Result<Option<T>, ConfigError>
where
    T: std::str::FromStr + PartialOrd + Default,
{
    get(var)
        .map(|v| match v.parse::<T>() {
            Ok(n) if n > T::default() => Ok(n),
            _ => Err(invalid(var, "must be a positive number")),
        })
        .transpose()
}

/// `DONKA_TRUSTED_PROXIES`: comma-separated addresses or CIDR ranges; none by default.
fn trusted_proxies(
    get: &impl Fn(&str) -> Option<String>,
) -> Result<Vec<ipnet::IpNet>, ConfigError> {
    let Some(list) = get("DONKA_TRUSTED_PROXIES") else {
        return Ok(Vec::new());
    };
    list.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .parse::<ipnet::IpNet>()
                .or_else(|_| entry.parse::<std::net::IpAddr>().map(ipnet::IpNet::from))
                .map_err(|_| {
                    invalid(
                        "DONKA_TRUSTED_PROXIES",
                        "must list IP addresses or CIDR ranges, e.g. 10.0.0.5,172.16.0.0/12",
                    )
                })
        })
        .collect()
}

fn boolean(
    get: &impl Fn(&str) -> Option<String>,
    var: &'static str,
    default: bool,
) -> Result<bool, ConfigError> {
    match get(var).as_deref() {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(invalid(var, "must be true or false")),
    }
}

fn validate_base_path(path: &str) -> Result<(), ConfigError> {
    let well_formed = path.len() > 1
        && path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains("//")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
    if well_formed {
        Ok(())
    } else {
        Err(invalid(
            "DONKA_API_BASE_PATH",
            "must start with '/', not end with '/', and use only letters, digits, '-', '_', '.'",
        ))
    }
}

fn invalid(var: &'static str, reason: &str) -> ConfigError {
    ConfigError {
        var,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const REQUIRED: [(&str, &str); 7] = [
        ("DATABASE_URL", "postgres://donka@localhost/donka"),
        ("DONKA_SMTP_URL", "smtp://localhost:1025"),
        ("DONKA_SMTP_FROM", "donka@bank.example"),
        ("DONKA_STORAGE_URL", "s3://donka-releases"),
        (
            "DONKA_DECISION_LOG_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
        ),
        (
            "DONKA_SECOND_STEP_KEY",
            "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=",
        ),
        (
            "DONKA_PERSON_INDEX_KEY",
            "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=",
        ),
    ];

    /// Loads with the required variables set, unless the test overrides them.
    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> = REQUIRED
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        map.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        Config::from_lookup(|k| map.get(k).cloned())
    }

    #[test]
    fn defaults_work_without_any_variable() {
        let config = load(&[]).unwrap();
        assert_eq!(config.api_base_path, "/api/v1");
        assert_eq!(config.listen.port(), 8080);
        assert_eq!(config.engine_workers, None);
        assert!(config.db_migrate);
        assert_eq!(config.db_max_connections, 10);
        assert!(config.cookie_secure);
        assert!(!config.otel_enabled);
        assert_eq!(config.session_idle_minutes, 480);
        assert_eq!(config.sign_in_max_failures, 5);
        assert_eq!(config.sign_in_lock_minutes, 15);
        assert_eq!(config.public_url, "http://localhost:8080");
        assert_eq!(config.bootstrap_admin_email, None);
        assert_eq!(config.default_locale, Locale::En);
        assert_eq!(config.invitation_link_hours, 72);
        assert_eq!(config.password_reset_link_minutes, 30);
        assert_eq!(config.email_max_attempts, 10);
        assert_eq!(config.web_dir, None);
        assert_eq!(config.decision_log_retention_days, 1825);
    }

    #[test]
    fn the_decision_log_key_is_required_and_checked() {
        for bad in ["", "too-short", "AAECAwQFBgcICQoLDA0ODw=="] {
            assert_eq!(
                load(&[("DONKA_DECISION_LOG_KEY", bad)]).unwrap_err().var,
                "DONKA_DECISION_LOG_KEY",
                "{bad}"
            );
        }
        assert_eq!(
            load(&[("DONKA_DECISION_LOG_RETENTION_DAYS", "0")])
                .unwrap_err()
                .var,
            "DONKA_DECISION_LOG_RETENTION_DAYS"
        );
        let config = load(&[("DONKA_DECISION_LOG_RETENTION_DAYS", "365")]).unwrap();
        assert_eq!(config.decision_log_retention_days, 365);
    }

    /// A file holding `content`, removed when the test ends.
    struct SecretFile(std::path::PathBuf);

    impl SecretFile {
        fn new(name: &str, content: &str) -> Self {
            let path = std::env::temp_dir().join(format!("donka-{}-{name}", std::process::id()));
            std::fs::write(&path, content).unwrap();
            Self(path)
        }

        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }

    impl Drop for SecretFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Loads with the required variables set, without `skip`, plus `vars`.
    fn load_without(skip: &str, vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> = REQUIRED
            .iter()
            .filter(|(k, _)| *k != skip)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        map.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        Config::from_lookup(|k| map.get(k).cloned())
    }

    #[test]
    fn every_secret_setting_can_be_a_file() {
        let key = "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=";
        let file = SecretFile::new("person-key", &format!("{key}\n\n"));
        let config = load_without(
            "DONKA_PERSON_INDEX_KEY",
            &[("DONKA_PERSON_INDEX_KEY_FILE", file.path())],
        )
        .unwrap();
        assert_eq!(
            config.person_index_key, key,
            "trailing newlines are trimmed"
        );

        let database = SecretFile::new("database", "postgres://donka:s3cret@db/donka\n");
        let config =
            load_without("DATABASE_URL", &[("DATABASE_URL_FILE", database.path())]).unwrap();
        assert_eq!(config.database_url, "postgres://donka:s3cret@db/donka");

        let storage = SecretFile::new("storage", "minio-secret");
        let config = load(&[("DONKA_STORAGE_SECRET_ACCESS_KEY_FILE", storage.path())]).unwrap();
        assert!(config.storage_options.contains(&(
            "aws_secret_access_key".to_owned(),
            "minio-secret".to_owned()
        )));

        // Every secret setting has its file form, and only secret settings do.
        for (var, file_var) in SECRET_SETTINGS {
            assert_eq!(file_var, format!("{var}_FILE"));
        }
    }

    #[test]
    fn a_secret_given_twice_or_an_unreadable_file_fails_and_is_named() {
        let file = SecretFile::new("twice", "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=");
        let err = load(&[("DONKA_PERSON_INDEX_KEY_FILE", file.path())]).unwrap_err();
        assert_eq!(err.var, "DONKA_PERSON_INDEX_KEY");
        assert!(
            err.reason
                .contains("DONKA_PERSON_INDEX_KEY_FILE are both set"),
            "{err}"
        );

        let err = load_without(
            "DONKA_SECOND_STEP_KEY",
            &[(
                "DONKA_SECOND_STEP_KEY_FILE",
                "/nonexistent/donka-second-step-key",
            )],
        )
        .unwrap_err();
        assert_eq!(err.var, "DONKA_SECOND_STEP_KEY_FILE");
        assert!(err.reason.starts_with("cannot be read"), "{err}");

        // A file holding a bad value is checked like the variable.
        let bad = SecretFile::new("bad-key", "too-short\n");
        let err = load_without(
            "DONKA_DECISION_LOG_KEY",
            &[("DONKA_DECISION_LOG_KEY_FILE", bad.path())],
        )
        .unwrap_err();
        assert_eq!(err.var, "DONKA_DECISION_LOG_KEY");
    }

    #[test]
    fn the_person_index_key_is_required_and_checked() {
        for bad in ["", "too-short", "AAECAwQFBgcICQoLDA0ODw=="] {
            assert_eq!(
                load(&[("DONKA_PERSON_INDEX_KEY", bad)]).unwrap_err().var,
                "DONKA_PERSON_INDEX_KEY",
                "{bad}"
            );
        }
    }

    #[test]
    fn the_second_step_key_is_required_and_checked() {
        for bad in ["", "too-short", "AAECAwQFBgcICQoLDA0ODw=="] {
            assert_eq!(
                load(&[("DONKA_SECOND_STEP_KEY", bad)]).unwrap_err().var,
                "DONKA_SECOND_STEP_KEY",
                "{bad}"
            );
        }
        let previous = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
        let config = load(&[("DONKA_SECOND_STEP_PREVIOUS_KEYS", previous)]).unwrap();
        assert_eq!(config.second_step_previous_keys, vec![previous]);
        assert_eq!(
            load(&[("DONKA_SECOND_STEP_PREVIOUS_KEYS", "nope")])
                .unwrap_err()
                .var,
            "DONKA_SECOND_STEP_PREVIOUS_KEYS"
        );
    }

    #[test]
    fn the_second_step_policy_defaults_to_none() {
        assert_eq!(load(&[]).unwrap().mfa_required, MfaRequired::None);
        for (value, policy) in [
            ("none", MfaRequired::None),
            ("privileged", MfaRequired::Privileged),
            ("all", MfaRequired::All),
        ] {
            assert_eq!(
                load(&[("DONKA_MFA_REQUIRED", value)]).unwrap().mfa_required,
                policy
            );
        }
        assert_eq!(
            load(&[("DONKA_MFA_REQUIRED", "admins")]).unwrap_err().var,
            "DONKA_MFA_REQUIRED"
        );
    }

    #[test]
    fn the_packs_folder_is_optional() {
        assert_eq!(load(&[]).unwrap().packs_dir, None);
        assert_eq!(
            load(&[("DONKA_PACKS_DIR", "/srv/packs")])
                .unwrap()
                .packs_dir,
            Some(PathBuf::from("/srv/packs"))
        );
    }

    #[test]
    fn the_licence_file_is_optional() {
        assert_eq!(load(&[]).unwrap().licence_file, None);
        assert_eq!(
            load(&[("DONKA_LICENCE_FILE", "/etc/donka/licence.json")])
                .unwrap()
                .licence_file,
            Some(PathBuf::from("/etc/donka/licence.json"))
        );
    }

    #[test]
    fn the_demo_is_off_unless_asked_for() {
        assert!(!load(&[]).unwrap().demo);
        assert!(load(&[("DONKA_DEMO", "true")]).unwrap().demo);
        assert_eq!(
            load(&[("DONKA_DEMO", "maybe")]).unwrap_err().var,
            "DONKA_DEMO"
        );
    }

    #[test]
    fn the_runtime_log_token_file_is_optional() {
        assert_eq!(load(&[]).unwrap().runtime_log_token_file, None);
        let file = "/var/lib/donka/wiring/staging-decision-log-token";
        assert_eq!(
            load(&[("DONKA_RUNTIME_LOG_TOKEN_FILE", file)])
                .unwrap()
                .runtime_log_token_file,
            Some(PathBuf::from(file))
        );
    }

    #[test]
    fn previous_decision_log_keys_are_optional_and_checked() {
        assert!(load(&[]).unwrap().decision_log_previous_keys.is_empty());
        let (a, b) = (
            "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
            "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=",
        );
        let config =
            load(&[("DONKA_DECISION_LOG_PREVIOUS_KEYS", &format!(" {a},\n{b} "))]).unwrap();
        assert_eq!(config.decision_log_previous_keys, vec![a, b]);
        assert_eq!(
            load(&[("DONKA_DECISION_LOG_PREVIOUS_KEYS", &format!("{a},nope"))])
                .unwrap_err()
                .var,
            "DONKA_DECISION_LOG_PREVIOUS_KEYS"
        );
    }

    #[test]
    fn explanations_are_off_unless_an_endpoint_is_configured() {
        assert_eq!(load(&[]).unwrap().explain, None);
        assert_eq!(
            load(&[("DONKA_EXPLAIN_API_KEY", "k")]).unwrap_err().var,
            "DONKA_EXPLAIN_URL"
        );
        let on = [
            (
                "DONKA_EXPLAIN_URL",
                "https://llm.bank.example/v1/chat/completions",
            ),
            ("DONKA_EXPLAIN_API_KEY", "k"),
            ("DONKA_EXPLAIN_API", "openai"),
            ("DONKA_EXPLAIN_MODEL", "local-model"),
        ];
        let explain = load(&on).unwrap().explain.unwrap();
        assert_eq!(explain.protocol, donka_explain::Protocol::OpenAi);
        assert_eq!(explain.model, "local-model");
        assert_eq!(explain.timeout, std::time::Duration::from_secs(60));
        assert!(!explain.fallbacks);

        // Each of the others is required once the endpoint is set, and checked.
        for (missing, _) in &on[1..] {
            let vars: Vec<_> = on.iter().copied().filter(|(k, _)| k != missing).collect();
            assert_eq!(load(&vars).unwrap_err().var, *missing);
        }
        let with = |extra: (&'static str, &'static str)| {
            let mut vars = on.to_vec();
            vars.retain(|(k, _)| *k != extra.0);
            vars.push(extra);
            load(&vars)
        };
        for bad in [
            ("DONKA_EXPLAIN_URL", "llm.bank.example"),
            ("DONKA_EXPLAIN_API", "gpt"),
            ("DONKA_EXPLAIN_TIMEOUT_SECONDS", "0"),
            ("DONKA_EXPLAIN_FALLBACKS", "true"),
        ] {
            assert_eq!(with(bad).unwrap_err().var, bad.0, "{bad:?}");
        }
        let anthropic = load(&[
            ("DONKA_EXPLAIN_URL", "https://api.anthropic.com/v1/messages"),
            ("DONKA_EXPLAIN_API_KEY", "k"),
            ("DONKA_EXPLAIN_API", "anthropic"),
            ("DONKA_EXPLAIN_MODEL", "claude-opus-5-5"),
            ("DONKA_EXPLAIN_FALLBACKS", "true"),
            ("DONKA_EXPLAIN_TIMEOUT_SECONDS", "90"),
        ])
        .unwrap()
        .explain
        .unwrap();
        assert!(anthropic.fallbacks);
        assert_eq!(anthropic.timeout, std::time::Duration::from_secs(90));
        assert!(
            !format!("{anthropic:?}").contains("\"k\""),
            "the key is never printed"
        );
    }

    #[test]
    fn storage_is_required_and_takes_s3_settings() {
        let config = load(&[]).unwrap();
        assert_eq!(config.storage_url, "s3://donka-releases");
        assert!(config.storage_options.is_empty());
        assert_eq!(config.publish_max_attempts, 10);
        for bad in [" ", "https://bucket", "/var/lib/releases"] {
            assert_eq!(
                load(&[("DONKA_STORAGE_URL", bad)]).unwrap_err().var,
                "DONKA_STORAGE_URL",
                "{bad}"
            );
        }
        let minio = load(&[
            ("DONKA_STORAGE_ENDPOINT", "http://localhost:9000"),
            ("DONKA_STORAGE_REGION", "us-east-1"),
            ("DONKA_STORAGE_ACCESS_KEY_ID", "donka"),
            ("DONKA_STORAGE_SECRET_ACCESS_KEY", "donka-secret"),
            ("DONKA_PUBLISH_MAX_ATTEMPTS", "3"),
        ])
        .unwrap();
        assert!(minio
            .storage_options
            .contains(&("aws_allow_http".into(), "true".into())));
        assert!(minio
            .storage_options
            .contains(&("aws_endpoint".into(), "http://localhost:9000".into())));
        assert_eq!(minio.storage_options.len(), 5);
        assert_eq!(minio.publish_max_attempts, 3);
        assert_eq!(
            load(&[("DONKA_PUBLISH_MAX_ATTEMPTS", "0")])
                .unwrap_err()
                .var,
            "DONKA_PUBLISH_MAX_ATTEMPTS"
        );
    }

    #[test]
    fn the_web_app_needs_the_api_where_it_calls_it() {
        let config = load(&[("DONKA_WEB_DIR", "/srv/web")]).unwrap();
        assert_eq!(config.web_dir, Some(PathBuf::from("/srv/web")));
        let err = load(&[
            ("DONKA_WEB_DIR", "/srv/web"),
            ("DONKA_API_BASE_PATH", "/studio/api"),
        ])
        .unwrap_err();
        assert_eq!(err.var, "DONKA_API_BASE_PATH");
    }

    #[test]
    fn email_settings_are_required_and_validated() {
        for var in ["DONKA_SMTP_URL", "DONKA_SMTP_FROM"] {
            assert_eq!(load(&[(var, " ")]).unwrap_err().var, var);
        }
        assert_eq!(
            load(&[("DONKA_DEFAULT_LOCALE", "fr")])
                .unwrap()
                .default_locale,
            Locale::Fr
        );
        for (var, bad) in [
            ("DONKA_DEFAULT_LOCALE", "de"),
            ("DONKA_INVITATION_LINK_HOURS", "0"),
            ("DONKA_PASSWORD_RESET_LINK_MINUTES", "-5"),
            ("DONKA_EMAIL_MAX_ATTEMPTS", "many"),
        ] {
            assert_eq!(load(&[(var, bad)]).unwrap_err().var, var, "{var}={bad}");
        }
    }

    #[test]
    fn sign_in_settings_are_validated() {
        assert!(
            !load(&[("DONKA_COOKIE_SECURE", "false")])
                .unwrap()
                .cookie_secure
        );
        for (var, bad) in [
            ("DONKA_COOKIE_SECURE", "yes"),
            ("DONKA_SESSION_IDLE_MINUTES", "0"),
            ("DONKA_SIGN_IN_MAX_FAILURES", "-1"),
            ("DONKA_SIGN_IN_LOCK_MINUTES", "soon"),
            ("DONKA_PUBLIC_URL", "studio.bank.example"),
            ("DONKA_PUBLIC_URL", "https://studio.bank.example/"),
        ] {
            assert_eq!(load(&[(var, bad)]).unwrap_err().var, var, "{var}={bad}");
        }
    }

    #[test]
    fn database_url_is_required() {
        let err = Config::from_lookup(|_| None).unwrap_err();
        assert_eq!(err.var, "DATABASE_URL");
        assert_eq!(
            load(&[("DATABASE_URL", "  ")]).unwrap_err().var,
            "DATABASE_URL"
        );
    }

    #[test]
    fn database_settings_are_validated() {
        assert!(!load(&[("DONKA_DB_MIGRATE", "false")]).unwrap().db_migrate);
        assert_eq!(
            load(&[("DONKA_DB_MIGRATE", "no")]).unwrap_err().var,
            "DONKA_DB_MIGRATE"
        );
        assert_eq!(
            load(&[("DONKA_DB_MAX_CONNECTIONS", "0")]).unwrap_err().var,
            "DONKA_DB_MAX_CONNECTIONS"
        );
    }

    #[test]
    fn base_path_can_be_changed() {
        let config = load(&[("DONKA_API_BASE_PATH", "/studio/api/v2")]).unwrap();
        assert_eq!(config.api_base_path, "/studio/api/v2");
    }

    #[test]
    fn a_malformed_base_path_names_the_variable() {
        for bad in ["api/v1", "/api/v1/", "/", "/api//v1", "/api v1"] {
            let err = load(&[("DONKA_API_BASE_PATH", bad)]).unwrap_err();
            assert_eq!(err.var, "DONKA_API_BASE_PATH", "{bad}");
        }
    }

    #[test]
    fn rate_limits_and_trusted_proxies() {
        let config = load(&[]).unwrap();
        assert_eq!(config.sign_in_rate_limit, 10);
        assert_eq!(config.password_reset_rate_limit, 5);
        assert!(config.trusted_proxies.is_empty());

        let config = load(&[
            ("DONKA_SIGN_IN_RATE_LIMIT", "3"),
            ("DONKA_PASSWORD_RESET_RATE_LIMIT", "1"),
            ("DONKA_TRUSTED_PROXIES", "10.0.0.5, 172.16.0.0/12,::1"),
        ])
        .unwrap();
        assert_eq!(config.sign_in_rate_limit, 3);
        assert_eq!(config.password_reset_rate_limit, 1);
        let proxies: Vec<String> = config
            .trusted_proxies
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(proxies, ["10.0.0.5/32", "172.16.0.0/12", "::1/128"]);

        assert_eq!(
            load(&[("DONKA_TRUSTED_PROXIES", "10.0.0.0/33")])
                .unwrap_err()
                .var,
            "DONKA_TRUSTED_PROXIES"
        );
        assert_eq!(
            load(&[("DONKA_SIGN_IN_RATE_LIMIT", "0")]).unwrap_err().var,
            "DONKA_SIGN_IN_RATE_LIMIT"
        );
    }

    #[test]
    fn invalid_numbers_and_addresses_name_the_variable() {
        assert_eq!(
            load(&[("DONKA_ENGINE_WORKERS", "0")]).unwrap_err().var,
            "DONKA_ENGINE_WORKERS"
        );
        assert_eq!(
            load(&[("DONKA_LISTEN", "8080")]).unwrap_err().var,
            "DONKA_LISTEN"
        );
    }
}
