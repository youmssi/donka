use std::net::SocketAddr;

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
}

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

        let database_url = get("DATABASE_URL")
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                invalid(
                    "DATABASE_URL",
                    "is required, e.g. postgres://user:pass@host:5432/donka",
                )
            })?;

        let public_url = get("DONKA_PUBLIC_URL").unwrap_or_else(|| "http://localhost:8080".into());
        if !(public_url.starts_with("http://") || public_url.starts_with("https://"))
            || public_url.ends_with('/')
        {
            return Err(invalid(
                "DONKA_PUBLIC_URL",
                "must start with http:// or https:// and not end with '/'",
            ));
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
        })
    }
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

    const DB: (&str, &str) = ("DATABASE_URL", "postgres://donka@localhost/donka");

    /// Loads with a database URL, the only required variable, unless the test overrides it.
    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> = HashMap::from([(DB.0.into(), DB.1.into())]);
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
