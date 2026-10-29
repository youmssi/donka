//! `donka-admin`: the commands an operator runs on the server (DNK-64, DNK-74). It ships in
//! the Studio image next to `donka-app`.
//!
//! ```text
//! docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
//!   ghcr.io/youmssi/donka-studio:<version> donka-admin init [--demo]
//! ```

use donka_app::install::{self, Options, Outcome};
use donka_app::upgrade;
use std::path::PathBuf;

const USAGE: &str = "usage:
  donka-admin init [--dir <dir>] [--public-url <url>] [--admin-email <email>]
                   [--studio-version <tag>] [--runtime-version <tag>] [--registry <host/path>]
                   [--demo]
      Prepares an installation in <dir> (the current folder by default): docker-compose.yml
      and .env with random secrets. Existing files are kept, never overwritten.
      --demo also releases the retail-credit pack to staging at the first start, its bureau
      connector answered by a fake bureau (its key in runtime.env).
      --registry names a mirror of the images (ghcr.io/youmssi by default).
      Then: docker compose up -d
  donka-admin upgrade <studio version> [--runtime-version <tag>] [--key <file>]
      Moves the installation to another release. Run it with the INSTALLED version's image, in
      the installation folder, with Docker's socket:
        docker run --rm -u \"$(id -u):$(id -g)\" --group-add \"$(stat -c %g /var/run/docker.sock)\" \\
          -v /var/run/docker.sock:/var/run/docker.sock -v \"$PWD:/install\" -w /install \\
          -e DONKA_REGISTRY_USER -e DONKA_REGISTRY_TOKEN \\
          ghcr.io/youmssi/donka-studio:<installed version> donka-admin upgrade <version>
      It verifies the new images with --key (cosign.pub in the folder by default), checks that
      the release accepts this installation, backs up to backups/, upgrades the Runtime then
      Studio and checks each, and restores everything if a check fails. Its report is written
      to upgrades/. Exit status: 0 upgraded, 3 refused (nothing changed), 4 rolled back,
      5 the restore failed too.
  donka-admin demo-bureau
      The demo's fake credit bureau, on port 8080 (the demo-bureau service runs it).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("init") => init(&args[1..]),
        Some("upgrade") => run_upgrade(&args[1..]),
        Some("demo-bureau") if args.len() == 1 => demo_bureau(),
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
        if flag == "--demo" {
            options.demo = true;
            continue;
        }
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
            "--registry" => options.registry = Some(value),
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

fn run_upgrade(args: &[String]) -> Result<(), String> {
    let mut rest = args.iter();
    let studio = rest
        .next()
        .filter(|version| !version.starts_with('-'))
        .ok_or_else(|| format!("which version?\n\n{USAGE}"))?
        .clone();
    let mut options = upgrade::Options {
        dir: PathBuf::from("."),
        studio,
        runtime: None,
        key: PathBuf::from("cosign.pub"),
        credentials: None,
    };
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n\n{USAGE}"))?
            .clone();
        match flag.as_str() {
            "--runtime-version" => options.runtime = Some(value),
            "--key" => options.key = PathBuf::from(value),
            _ => return Err(format!("unknown option `{flag}`\n\n{USAGE}")),
        }
    }
    let variable = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    options.credentials = variable("DONKA_REGISTRY_TOKEN").map(|token| {
        (
            variable("DONKA_REGISTRY_USER").unwrap_or_else(|| "donka".to_owned()),
            token,
        )
    });
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    let report = runtime.block_on(upgrade::run(options))?;
    let code = match report.outcome {
        upgrade::Outcome::Upgraded => 0,
        upgrade::Outcome::Refused => 3,
        upgrade::Outcome::RolledBack => 4,
        upgrade::Outcome::RestoreFailed => 5,
    };
    std::process::exit(code)
}

/// Serves the demo's fake bureau until the container stops.
fn demo_bureau() -> Result<(), String> {
    let key = std::env::var("DONKA_SECRET_BUREAU_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or("DONKA_SECRET_BUREAU_API_KEY is not set: run donka-admin init --demo")?;
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
            .await
            .map_err(|err| format!("cannot listen on port 8080: {err}"))?;
        println!("demo bureau listening on port 8080");
        axum::serve(listener, donka_app::demo::bureau::router(key))
            .await
            .map_err(|err| err.to_string())
    })
}
