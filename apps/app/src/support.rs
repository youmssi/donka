//! `donka-admin support-bundle` (DNK-74): what Donka's support needs to understand an
//! installation, in one file the operator can read before sending it. Versions, the
//! configuration, the doctor's report, recent logs and the latest upgrade reports, with every
//! secret, token, email address and public IP address removed. Support never needs access to
//! the bank's servers.

use crate::install::{self, CONFIG, SECRETS};
use crate::upgrade::docker::Engine;
use flate2::write::GzEncoder;
use flate2::Compression;
use regex::{Captures, Regex};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Logs of the last day, at most this many lines per service.
const LOG_LINES: usize = 5000;
/// The latest upgrade reports.
const UPGRADE_REPORTS: usize = 3;
/// The configuration files, as Compose reads them.
const CONFIG_FILES: [&str; 5] = [
    CONFIG,
    ".env",
    "runtime.env",
    "docker-compose.yml",
    "docker-compose.override.yml",
];

pub struct Options {
    /// The installation folder.
    pub dir: PathBuf,
    /// How far back logs go.
    pub since_hours: i64,
}

/// One file of the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub text: String,
}

/// Collects, cleans and writes the bundle into the installation folder; returns its path.
pub async fn run(options: &Options) -> Result<PathBuf, String> {
    let secrets = secret_values(&options.dir.join(SECRETS))?;
    let mut entries = files(&options.dir)?;
    match Engine::connect().await {
        Ok(engine) => entries.extend(containers(&engine, options.since_hours).await),
        Err(err) => entries.push(Entry {
            name: "containers.txt".to_owned(),
            text: format!("Docker could not be asked: {err}\n"),
        }),
    }
    let entries: Vec<Entry> = entries
        .into_iter()
        .map(|entry| Entry {
            text: redact(&entry.text, &secrets),
            name: entry.name,
        })
        .collect();
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let path = options.dir.join(format!("support-{stamp}.tar.gz"));
    let bytes = archive(&readme(), &entries)?;
    std::fs::write(&path, bytes)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    restrict(&path)?;
    Ok(path)
}

fn readme() -> String {
    format!(
        "Donka support bundle, made by donka-admin {} on {}.\n\n\
         versions.txt       the installation's images and containers\n\
         config/            donka.yaml and the files Compose reads\n\
         secrets.txt        the names of the secret files, never their content\n\
         doctor.txt         what `donka-admin doctor` found\n\
         logs/              each service's recent log\n\
         upgrades/          the latest upgrade reports\n\n\
         Removed everywhere: the values of the secret files, passwords in addresses, Donka \
         tokens, other secret settings, email addresses and public IP addresses. Read it before \
         sending it.\n",
        crate::VERSION,
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    )
}

/// The configuration files, the secrets' names and the latest upgrade reports.
fn files(dir: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for name in CONFIG_FILES {
        if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
            entries.push(Entry {
                name: format!("config/{name}"),
                text,
            });
        }
    }
    let mut names: Vec<String> = std::fs::read_dir(dir.join(SECRETS))
        .map(|files| {
            files
                .flatten()
                .map(|file| file.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    entries.push(Entry {
        name: "secrets.txt".to_owned(),
        text: names.join("\n") + "\n",
    });
    let mut reports: Vec<PathBuf> = std::fs::read_dir(dir.join("upgrades"))
        .map(|files| {
            files
                .flatten()
                .map(|file| file.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
                .collect()
        })
        .unwrap_or_default();
    // Named by time: the last ones are the latest.
    reports.sort();
    for report in reports.iter().rev().take(UPGRADE_REPORTS) {
        if let (Some(name), Ok(text)) = (report.file_name(), std::fs::read_to_string(report)) {
            entries.push(Entry {
                name: format!("upgrades/{}", name.to_string_lossy()),
                text,
            });
        }
    }
    Ok(entries)
}

/// The containers, their logs, and the doctor's report from Studio's container.
async fn containers(engine: &Engine, since_hours: i64) -> Vec<Entry> {
    let containers = match engine.containers().await {
        Ok(containers) => containers,
        Err(err) => {
            return vec![Entry {
                name: "versions.txt".to_owned(),
                text: format!("The containers could not be listed: {err}\n"),
            }]
        }
    };
    let mut versions = format!("donka-admin {}\n\n", crate::VERSION);
    let mut entries = Vec::new();
    let mut doctor = "Studio's container is not running: the doctor could not run.\n".to_owned();
    for container in &containers {
        versions.push_str(&format!(
            "{:<14} {}  {}  {}\n",
            container.service, container.image, container.image_id, container.status
        ));
        let log = engine
            .logs(&container.id, since_hours * 3600, LOG_LINES)
            .await
            .unwrap_or_else(|err| format!("The log could not be read: {err}\n"));
        entries.push(Entry {
            name: format!("logs/{}.log", container.service),
            text: log,
        });
        if container.service == "app" && container.running {
            doctor = engine
                .exec(&container.id, &["donka-admin", "doctor"])
                .await
                .unwrap_or_else(|err| format!("The doctor could not run: {err}\n"));
        }
    }
    entries.push(Entry {
        name: "versions.txt".to_owned(),
        text: versions,
    });
    entries.push(Entry {
        name: "doctor.txt".to_owned(),
        text: doctor,
    });
    entries
}

/// The values in the secret files, longest first so a value inside another one goes last.
fn secret_values(secrets: &Path) -> Result<Vec<String>, String> {
    let Ok(files) = std::fs::read_dir(secrets) else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();
    for file in files.flatten() {
        let Ok(value) = install::read_secret(&file.path()) else {
            continue;
        };
        values.extend(
            value
                .lines()
                .map(str::trim)
                .filter(|line| line.len() >= 6)
                .map(str::to_owned),
        );
        // A URL's password alone, e.g. the database's in DATABASE_URL.
        if let Some(captures) = URL_CREDENTIALS.captures(&value) {
            values.push(captures[3].to_owned());
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    Ok(values)
}

static URL_CREDENTIALS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://)([^/\s:@]+):([^/\s@]+)@").expect("valid pattern")
});
static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bdnk_[A-Za-z0-9_-]{8,}").expect("valid pattern"));
static BEARER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/-]+=*").expect("valid pattern")
});
static QUERY_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([?&](?:token|password|secret|key|code)=)[^&\s#]+").expect("valid pattern")
});
/// `NAME=value` and `NAME: value` whose name says it holds a secret, unless it names a file.
static SECRET_SETTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^(\s*-?\s*["']?([A-Za-z0-9_]*(?:SECRET|PASSWORD|PASSWD|TOKEN|KEY|CREDENTIAL)[A-Za-z0-9_]*)["']?\s*[=:]\s*)(\S.*)$"#,
    )
    .expect("valid pattern")
});
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
        .expect("valid pattern")
});
static IPV4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("valid pattern"));

/// `text` without secrets, tokens, email addresses or public IP addresses.
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut text = text.to_owned();
    for secret in secrets {
        text = text.replace(secret.as_str(), "[secret]");
    }
    let text = URL_CREDENTIALS.replace_all(&text, "$1[redacted]@");
    let text = TOKEN.replace_all(&text, "[token]");
    let text = BEARER.replace_all(&text, "$1 [redacted]");
    let text = QUERY_SECRET.replace_all(&text, "$1[redacted]");
    let text = SECRET_SETTING.replace_all(&text, |captures: &Captures| {
        let name = &captures[2];
        let value = &captures[3];
        // A file's path, an interpolation, or already redacted: nothing secret in it.
        if name.ends_with("_FILE") || value.starts_with(['/', '$', '[']) {
            captures[0].to_owned()
        } else {
            format!("{}[redacted]", &captures[1])
        }
    });
    let text = EMAIL.replace_all(&text, "[email]");
    IPV4.replace_all(&text, |captures: &Captures| {
        let address = &captures[0];
        match address.parse::<Ipv4Addr>() {
            Ok(ip) if !(ip.is_private() || ip.is_loopback() || ip.is_unspecified()) => {
                "[ip]".to_owned()
            }
            _ => address.to_owned(),
        }
    })
    .into_owned()
}

/// The README and the entries as a `.tar.gz`, under a `donka-support/` folder.
pub fn archive(readme: &str, entries: &[Entry]) -> Result<Vec<u8>, String> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let readme = Entry {
        name: "README.txt".to_owned(),
        text: readme.to_owned(),
    };
    for entry in std::iter::once(&readme).chain(entries) {
        let mut header = tar::Header::new_gnu();
        header.set_size(entry.text.len() as u64);
        header.set_mode(0o600);
        header.set_mtime(u64::try_from(chrono::Utc::now().timestamp()).unwrap_or(0));
        header.set_cksum();
        tar.append_data(
            &mut header,
            format!("donka-support/{}", entry.name),
            entry.text.as_bytes(),
        )
        .map_err(|err| err.to_string())?;
    }
    tar.into_inner()
        .and_then(GzEncoder::finish)
        .map_err(|err| err.to_string())
}

/// The bundle is the operator's to read and send: nobody else on the server reads it.
fn restrict(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("cannot restrict {}: {err}", path.display()))
}

#[cfg(test)]
mod tests;
