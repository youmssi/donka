//! `donka-admin`: the commands an operator runs on the server (DNK-64, DNK-74). It ships in
//! the Studio image next to `donka-app`.
//!
//! ```text
//! docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
//!   ghcr.io/youmssi/donka-studio:<version> donka-admin init
//! ```

use donka_app::install::{self, Options, Outcome};
use std::path::PathBuf;

const USAGE: &str = "usage:
  donka-admin init [--dir <dir>] [--public-url <url>] [--admin-email <email>]
                   [--studio-version <tag>] [--runtime-version <tag>]
      Prepares an installation in <dir> (the current folder by default): docker-compose.yml
      and .env with random secrets. Existing files are kept, never overwritten.
      Then: docker compose up -d";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("init") => init(&args[1..]),
        _ => Err(USAGE.to_owned()),
    };
    if let Err(err) = done {
        eprintln!("{err}");
        std::process::exit(2);
    }
}

fn init(args: &[String]) -> Result<(), String> {
    let mut options = Options::for_version(donka_app::VERSION);
    let mut dir = PathBuf::from(".");
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n\n{USAGE}"))?
            .clone();
        match flag.as_str() {
            "--dir" => dir = PathBuf::from(value),
            "--public-url" => options.public_url = value,
            "--admin-email" => options.admin_email = value,
            "--studio-version" => options.studio_version = value,
            "--runtime-version" => options.runtime_version = value,
            _ => return Err(format!("unknown option `{flag}`\n\n{USAGE}")),
        }
    }
    let outcomes = install::init(&dir, &options, |buf| {
        getrandom::fill(buf).map_err(|err| format!("no secure random source: {err}"))
    })?;
    for outcome in &outcomes {
        match outcome {
            Outcome::Written(path) => println!("written  {}", path.display()),
            Outcome::Kept(path) => {
                println!("kept     {} (it exists; nothing changed)", path.display())
            }
        }
    }
    println!(
        "\nNext:\n  docker compose up -d\n  docker compose logs app | grep setup-password\n\
         Then open {} and choose {}'s password with that link.",
        options.public_url, options.admin_email
    );
    Ok(())
}
