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
    pub session_idle_minutes: u32,
    pub sign_in_max_failures: u32,
    pub sign_in_lock_minutes: u32,
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
}

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

    /// Reads settings through `get` so tests do not touch the process environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
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

        Ok(Self {
            listen,
            engine_workers: positive(&get, "DONKA_ENGINE_WORKERS")?,
            api_base_path,
            database_url,
            db_migrate: boolean(&get, "DONKA_DB_MIGRATE", true)?,
            db_max_connections: positive(&get, "DONKA_DB_MAX_CONNECTIONS")?.unwrap_or(10),
            public_url,
            cookie_secure: boolean(&get, "DONKA_COOKIE_SECURE", true)?,
            session_idle_minutes: positive(&get, "DONKA_SESSION_IDLE_MINUTES")?.unwrap_or(480),
            sign_in_max_failures: positive(&get, "DONKA_SIGN_IN_MAX_FAILURES")?.unwrap_or(5),
            sign_in_lock_minutes: positive(&get, "DONKA_SIGN_IN_LOCK_MINUTES")?.unwrap_or(15),
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
        })
    }
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

    const REQUIRED: [(&str, &str); 4] = [
        ("DATABASE_URL", "postgres://donka@localhost/donka"),
        ("DONKA_SMTP_URL", "smtp://localhost:1025"),
        ("DONKA_SMTP_FROM", "donka@bank.example"),
        ("DONKA_STORAGE_URL", "s3://donka-releases"),
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
