//! `donka-licence`: issues Studio licence files (DNK-77).
//!
//! It runs at Donka, on a machine that holds the signing key, and is never built into the
//! Studio image. Studio only carries the public key (`crates/licence/keys/licence.pub`).
//!
//! ```text
//! donka-licence keygen <dir>
//! donka-licence issue --key <file> --customer <name> --from <YYYY-MM-DD> --until <YYYY-MM-DD>
//!                     [--module explain|connectors|analysis]... [--out <file>]
//! donka-licence inspect <licence file> --public-key <file>
//! ```

use anyhow::{bail, Context as _};
use chrono::{NaiveDate, Utc};
use donka_licence::{Licence, Module, PublicKey, SigningKeyPair, Status};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const USAGE: &str = "usage:
  donka-licence keygen <dir>
      Writes a new key pair: <dir>/licence.key (keep it offline) and <dir>/licence.pub
      (commit it as crates/licence/keys/licence.pub).
  donka-licence issue --key <file> --customer <name> --from <YYYY-MM-DD> --until <YYYY-MM-DD>
                      [--module explain|connectors|analysis]... [--out <file>]
      Signs a licence. Days are UTC and both are included. Without --out, prints it.
  donka-licence inspect <licence file> --public-key <file>
      Verifies a licence with a public key and shows what it says.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("keygen") => keygen(&args[1..]),
        Some("issue") => issue(&args[1..]),
        Some("inspect") => inspect(&args[1..]),
        _ => Err(anyhow::anyhow!("{USAGE}")),
    };
    if let Err(err) = done {
        eprintln!("{err:#}");
        std::process::exit(2);
    }
}

fn keygen(args: &[String]) -> anyhow::Result<()> {
    let [dir] = args else { bail!("{USAGE}") };
    let dir = Path::new(dir);
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|err| anyhow::anyhow!("no randomness: {err}"))?;
    let pair = SigningKeyPair::from_seed(seed);
    let private = dir.join("licence.key");
    let public = dir.join("licence.pub");
    write_new(&private, &pair.to_text(), true)?;
    write_new(&public, &pair.public_key().to_text(), false)?;
    println!(
        "key {}\n  signing key: {} (keep it offline; anyone holding it can issue licences)\n  \
         public key:  {} (commit it as crates/licence/keys/licence.pub)",
        pair.public_key().id(),
        private.display(),
        public.display()
    );
    Ok(())
}

fn issue(args: &[String]) -> anyhow::Result<()> {
    let mut key = None;
    let mut customer = None;
    let mut from = None;
    let mut until = None;
    let mut out = None;
    let mut modules = BTreeSet::new();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .with_context(|| format!("{flag} needs a value\n\n{USAGE}"))?;
        match flag.as_str() {
            "--key" => key = Some(PathBuf::from(value)),
            "--customer" => customer = Some(value.clone()),
            "--from" => from = Some(day(flag, value)?),
            "--until" => until = Some(day(flag, value)?),
            "--out" => out = Some(PathBuf::from(value)),
            "--module" => {
                let module = Module::parse(value).with_context(|| {
                    format!("unknown module `{value}`: explain, connectors or analysis")
                })?;
                modules.insert(module);
            }
            _ => bail!("unknown option `{flag}`\n\n{USAGE}"),
        }
    }
    let (Some(key), Some(customer), Some(from), Some(until)) = (key, customer, from, until) else {
        bail!("--key, --customer, --from and --until are required\n\n{USAGE}");
    };
    let pair =
        SigningKeyPair::from_text(&read(&key)?).with_context(|| format!("{}", key.display()))?;
    let licence = Licence {
        id: Uuid::new_v4(),
        customer: customer.trim().to_owned(),
        issued_at: Utc::now(),
        valid_from: from,
        valid_until: until,
        modules,
    };
    let file = pair.issue(&licence)?;
    match out {
        Some(out) => {
            write_new(&out, &file, false)?;
            eprintln!("licence {} written to {}", licence.id, out.display());
        }
        None => print!("{file}"),
    }
    Ok(())
}

fn inspect(args: &[String]) -> anyhow::Result<()> {
    let [file, flag, public] = args else {
        bail!("{USAGE}")
    };
    if flag != "--public-key" {
        bail!("{USAGE}");
    }
    let public = PublicKey::from_text(&read(Path::new(public))?)?;
    let licence = public.verify(&read(Path::new(file))?)?;
    let status = Status::on(Some(Ok(licence.clone())), Utc::now().date_naive());
    let modules: Vec<_> = licence.modules.iter().map(|m| m.as_str()).collect();
    println!(
        "licence  {}\ncustomer {}\nissued   {}\nvalid    {} to {} (UTC, both included)\nmodules  {}\ntoday    {}",
        licence.id,
        licence.customer,
        licence.issued_at.to_rfc3339(),
        licence.valid_from,
        licence.valid_until,
        if modules.is_empty() { "none".to_owned() } else { modules.join(", ") },
        match status {
            Status::Active { days_left, .. } => format!("valid, {days_left} more day(s)"),
            Status::NotYetValid { .. } => "not valid yet".to_owned(),
            Status::Expired { .. } => "expired".to_owned(),
            Status::Evaluation | Status::Refused { .. } => "not verified".to_owned(),
        }
    );
    Ok(())
}

fn day(flag: &str, value: &str) -> anyhow::Result<NaiveDate> {
    value
        .parse()
        .with_context(|| format!("{flag} `{value}`: expected YYYY-MM-DD"))
}

fn read(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

/// Writes a file that must not exist yet; a signing key is readable by its owner only.
fn write_new(path: &Path, content: &str, private: bool) -> anyhow::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot create {} (it must not exist yet)", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))
}
