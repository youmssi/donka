//! `donka-admin init` (DNK-64): prepares an installation from the published images, with
//! nothing but Docker on the server. It writes `docker-compose.yml` and `.env` with random
//! secrets into a folder, and never overwrites either: a second run keeps what is there.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The installation's compose file: Studio, the staging Runtime, PostgreSQL and Mailpit.
pub const COMPOSE: &str = include_str!("../install/docker-compose.yml");

#[derive(Debug, Clone)]
pub struct Options {
    /// Where people reach Studio.
    pub public_url: String,
    /// The first administrator, created on the empty database.
    pub admin_email: String,
    /// Image tags.
    pub studio_version: String,
    pub runtime_version: String,
    /// Where the images come from, when not GitHub Container Registry: a mirror.
    pub registry: Option<String>,
    /// No internet access: Studio never checks for new releases.
    pub offline: bool,
    /// The demo: the retail-credit pack released to staging at the first start, its bureau
    /// connector answered by a bundled fake bureau.
    pub demo: bool,
}

impl Options {
    /// The defaults for a first look on this machine, with the images of this Studio's own
    /// version: a release pins itself and the latest Runtime; any other build uses `edge`.
    pub fn for_version(version: &str) -> Self {
        let released = !version.is_empty()
            && version
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
        Self {
            public_url: "http://localhost:8080".to_owned(),
            admin_email: "admin@donka.local".to_owned(),
            studio_version: if released { version } else { "edge" }.to_owned(),
            runtime_version: if released { "latest" } else { "edge" }.to_owned(),
            registry: None,
            offline: false,
            demo: false,
        }
    }
}

/// What happened to each file.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Written(PathBuf),
    /// It existed: left as it was.
    Kept(PathBuf),
}

/// Writes the installation's files into `dir`. `random` fills a buffer with secure random
/// bytes.
pub fn init(
    dir: &Path,
    options: &Options,
    mut random: impl FnMut(&mut [u8]) -> Result<(), String>,
) -> Result<Vec<Outcome>, String> {
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let compose = write_once(
        &dir.join("docker-compose.yml"),
        || Ok(COMPOSE.to_owned()),
        false,
    )?;
    let env = write_once(&dir.join(".env"), || env_file(options, &mut random), true)?;
    let mut outcomes = vec![compose, env];
    if options.demo {
        let runtime = write_once(
            &dir.join("runtime.env"),
            || demo_runtime_env(&mut random),
            true,
        )?;
        outcomes.push(runtime);
    }
    Ok(outcomes)
}

/// The demo's `runtime.env`: the key the Runtime sends to the fake bureau, which reads the same
/// file. A real bureau's key replaces it there.
fn demo_runtime_env(
    random: &mut impl FnMut(&mut [u8]) -> Result<(), String>,
) -> Result<String, String> {
    let mut key = [0u8; 24];
    random(&mut key)?;
    let key: String = key.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "# Written once by `donka-admin init --demo`: the Runtime's settings and connector secrets.\n\
         # The key of the demo's fake credit bureau; replace it with your bureau's.\n\
         DONKA_SECRET_BUREAU_API_KEY={key}\n"
    ))
}

/// The `.env` file: the installation's settings and its generated secrets.
fn env_file(
    options: &Options,
    random: &mut impl FnMut(&mut [u8]) -> Result<(), String>,
) -> Result<String, String> {
    let mut key = || -> Result<String, String> {
        let mut bytes = [0u8; 32];
        random(&mut bytes)?;
        Ok(B64.encode(bytes))
    };
    let decision_log_key = key()?;
    let second_step_key = key()?;
    let person_index_key = key()?;
    let mut password = [0u8; 24];
    random(&mut password)?;
    let password: String = password.iter().map(|b| format!("{b:02x}")).collect();
    let Options {
        public_url,
        admin_email,
        studio_version,
        runtime_version,
        registry,
        offline,
        demo,
    } = options;
    let offline = if *offline {
        "\n# No internet access (donka-admin init --offline): Studio never checks for new releases\n\
         DONKA_UPDATE_CHECK=false\n"
    } else {
        ""
    };
    let registry = match registry {
        Some(registry) => format!("DONKA_REGISTRY={registry}"),
        None => "# A mirror of the images, if not GitHub Container Registry\n\
                 # DONKA_REGISTRY=ghcr.io/youmssi"
            .to_owned(),
    };
    let demo = if *demo {
        "\n# The demo (donka-admin init --demo): the retail-credit pack released to staging at the\n\
         # first start, and the fake bureau its connector calls\n\
         COMPOSE_PROFILES=demo\n\
         DONKA_DEMO=true\n"
    } else {
        ""
    };
    Ok(format!(
        "# Written once by `donka-admin init`. It holds this installation's secrets: keep it out of\n\
         # version control, back it up with the database, and never run init to replace it.\n\
         # Every other setting is in Studio's .env.example.\n\
         \n\
         # Images\n\
         DONKA_VERSION={studio_version}\n\
         DONKA_RUNTIME_VERSION={runtime_version}\n\
         {registry}\n\
         \n\
         # Where people reach Studio; behind HTTPS, also set DONKA_COOKIE_SECURE=true\n\
         DONKA_PUBLIC_URL={public_url}\n\
         DONKA_COOKIE_SECURE=false\n\
         # Created on the empty database; its one-time setup link is in the logs\n\
         DONKA_BOOTSTRAP_ADMIN_EMAIL={admin_email}\n\
         # A second sign-in step for administrators and owners (an account running the smoke test\n\
         # then needs one too): DONKA_MFA_REQUIRED=privileged\n\
         \n\
         # Generated secrets\n\
         DONKA_POSTGRES_PASSWORD={password}\n\
         DONKA_DECISION_LOG_KEY={decision_log_key}\n\
         DONKA_SECOND_STEP_KEY={second_step_key}\n\
         DONKA_PERSON_INDEX_KEY={person_index_key}\n\
         \n\
         # Mail: Mailpit (http://localhost:8025) catches every email until these point at your server\n\
         # DONKA_SMTP_URL=smtp://user:pass@mail.example:587?tls=required\n\
         # DONKA_SMTP_FROM=Donka <donka@example>\n\
         {offline}{demo}"
    ))
}

/// Creates `path` with `content` unless it exists. A file holding secrets is readable by its
/// owner only.
fn write_once(
    path: &Path,
    content: impl FnOnce() -> Result<String, String>,
    secret: bool,
) -> Result<Outcome, String> {
    if path.exists() {
        return Ok(Outcome::Kept(path.to_owned()));
    }
    let content = content()?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = secret;
    let mut file = options
        .open(path)
        .map_err(|err| format!("cannot create {}: {err}", path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    Ok(Outcome::Written(path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("donka-init-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn system_random(buf: &mut [u8]) -> Result<(), String> {
        getrandom::fill(buf).map_err(|err| err.to_string())
    }

    fn env_value(env: &str, key: &str) -> String {
        env.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("{key} missing"))
            .to_owned()
    }

    #[test]
    fn writes_the_compose_file_and_random_secrets() {
        let dir = dir("fresh");
        let outcome = init(&dir, &Options::for_version("1.4.0"), system_random).unwrap();
        assert!(matches!(
            outcome[..],
            [Outcome::Written(_), Outcome::Written(_)]
        ));
        assert_eq!(
            std::fs::read_to_string(dir.join("docker-compose.yml")).unwrap(),
            COMPOSE
        );
        let env = std::fs::read_to_string(dir.join(".env")).unwrap();
        assert_eq!(env_value(&env, "DONKA_VERSION"), "1.4.0");
        assert_eq!(env_value(&env, "DONKA_RUNTIME_VERSION"), "latest");
        for key in [
            "DONKA_DECISION_LOG_KEY",
            "DONKA_SECOND_STEP_KEY",
            "DONKA_PERSON_INDEX_KEY",
        ] {
            assert_eq!(B64.decode(env_value(&env, key)).unwrap().len(), 32, "{key}");
        }
        assert_eq!(env_value(&env, "DONKA_POSTGRES_PASSWORD").len(), 48);
        // Every line is a comment or KEY=value, with nothing after the value.
        for line in env.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            assert!(
                line.split_once('=')
                    .is_some_and(|(k, _)| k.starts_with("DONKA_")),
                "{line}"
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.join(".env"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_installation_gets_its_own_secrets() {
        let (a, b) = (dir("a"), dir("b"));
        let options = Options::for_version("1.4.0");
        init(&a, &options, system_random).unwrap();
        init(&b, &options, system_random).unwrap();
        let read = |d: &Path| std::fs::read_to_string(d.join(".env")).unwrap();
        let (a_env, b_env) = (read(&a), read(&b));
        let keys = [
            "DONKA_POSTGRES_PASSWORD",
            "DONKA_DECISION_LOG_KEY",
            "DONKA_SECOND_STEP_KEY",
            "DONKA_PERSON_INDEX_KEY",
        ];
        for key in keys {
            assert_ne!(env_value(&a_env, key), env_value(&b_env, key), "{key}");
        }
        // Within one installation, no two secrets are the same.
        let values: std::collections::BTreeSet<_> =
            keys.iter().map(|key| env_value(&a_env, key)).collect();
        assert_eq!(values.len(), keys.len());
        let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
    }

    #[test]
    fn a_second_run_overwrites_nothing() {
        let dir = dir("again");
        init(&dir, &Options::for_version("1.4.0"), system_random).unwrap();
        let before = std::fs::read_to_string(dir.join(".env")).unwrap();
        std::fs::write(dir.join("docker-compose.yml"), "# edited\n").unwrap();
        let outcome = init(&dir, &Options::for_version("2.0.0"), system_random).unwrap();
        assert!(matches!(outcome[..], [Outcome::Kept(_), Outcome::Kept(_)]));
        assert_eq!(std::fs::read_to_string(dir.join(".env")).unwrap(), before);
        assert_eq!(
            std::fs::read_to_string(dir.join("docker-compose.yml")).unwrap(),
            "# edited\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failing_random_source_writes_no_secrets_file() {
        let dir = dir("norandom");
        let failed = init(&dir, &Options::for_version("1.4.0"), |_| {
            Err("no entropy".into())
        });
        assert_eq!(failed.unwrap_err(), "no entropy");
        assert!(!dir.join(".env").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_demo_turns_on_its_profile_and_writes_the_bureau_key_once() {
        let plain = dir("plain");
        init(&plain, &Options::for_version("1.4.0"), system_random).unwrap();
        let env = std::fs::read_to_string(plain.join(".env")).unwrap();
        assert!(!env.contains("DONKA_DEMO") && !env.contains("COMPOSE_PROFILES"));
        assert!(!env.contains("DONKA_UPDATE_CHECK"));
        assert!(!plain.join("runtime.env").exists());

        let demo = dir("demo");
        let options = Options {
            demo: true,
            offline: true,
            registry: Some("registry.bank.example/donka".into()),
            ..Options::for_version("1.4.0")
        };
        let outcome = init(&demo, &options, system_random).unwrap();
        assert_eq!(outcome.len(), 3);
        let env = std::fs::read_to_string(demo.join(".env")).unwrap();
        assert_eq!(env_value(&env, "COMPOSE_PROFILES"), "demo");
        assert_eq!(env_value(&env, "DONKA_UPDATE_CHECK"), "false");
        assert_eq!(env_value(&env, "DONKA_DEMO"), "true");
        assert_eq!(
            env_value(&env, "DONKA_REGISTRY"),
            "registry.bank.example/donka"
        );
        let runtime = std::fs::read_to_string(demo.join("runtime.env")).unwrap();
        assert_eq!(env_value(&runtime, "DONKA_SECRET_BUREAU_API_KEY").len(), 48);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(demo.join("runtime.env"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        init(&demo, &options, system_random).unwrap();
        assert_eq!(
            std::fs::read_to_string(demo.join("runtime.env")).unwrap(),
            runtime
        );
        assert!(COMPOSE.contains("command: [donka-admin, demo-bureau]"));
        let _ = (
            std::fs::remove_dir_all(&plain),
            std::fs::remove_dir_all(&demo),
        );
    }

    #[test]
    fn a_build_that_is_not_a_release_uses_edge_images() {
        for version in ["unknown", "4078085f88d0", "1.4.0-rc.1", ""] {
            let options = Options::for_version(version);
            assert_eq!(
                (
                    options.studio_version.as_str(),
                    options.runtime_version.as_str()
                ),
                ("edge", "edge"),
                "{version}"
            );
        }
    }

    #[test]
    fn the_compose_file_wires_the_runtime_to_studio() {
        // Studio publishes where the Runtime watches, and writes the token where it reads it.
        assert!(COMPOSE.contains("DONKA_STORAGE_URL: file:///var/lib/donka/releases"));
        assert!(COMPOSE.contains("PROVIDER__ROOT_DIR: /var/lib/donka/releases/staging"));
        assert!(COMPOSE.contains("PROVIDER__WATCH: \"true\""));
        let token = "/var/lib/donka/wiring/staging-decision-log-token";
        assert!(COMPOSE.contains(&format!("DONKA_RUNTIME_LOG_TOKEN_FILE: {token}")));
        assert!(COMPOSE.contains(&format!("DECISION_LOG__TOKEN_FILE: {token}")));
    }
}
