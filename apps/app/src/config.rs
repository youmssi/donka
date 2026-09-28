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

        let engine_workers = get("DONKA_ENGINE_WORKERS")
            .map(|v| match v.parse::<usize>() {
                Ok(n) if n > 0 => Ok(n),
                _ => Err(invalid("DONKA_ENGINE_WORKERS", "must be a positive number")),
            })
            .transpose()?;

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

        let db_migrate = match get("DONKA_DB_MIGRATE").as_deref() {
            None | Some("true") => true,
            Some("false") => false,
            Some(_) => return Err(invalid("DONKA_DB_MIGRATE", "must be true or false")),
        };

        let db_max_connections = get("DONKA_DB_MAX_CONNECTIONS")
            .map(|v| match v.parse::<u32>() {
                Ok(n) if n > 0 => Ok(n),
                _ => Err(invalid(
                    "DONKA_DB_MAX_CONNECTIONS",
                    "must be a positive number",
                )),
            })
            .transpose()?
            .unwrap_or(10);

        Ok(Self {
            listen,
            engine_workers,
            api_base_path,
            database_url,
            db_migrate,
            db_max_connections,
        })
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
