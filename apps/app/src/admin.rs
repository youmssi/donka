//! `donka-admin`: the commands an operator runs on the server (DNK-64, DNK-74). It ships in
//! the Studio image next to `donka-app`.
//!
//! ```text
//! docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
//!   ghcr.io/youmssi/donka-studio:<version> donka-admin init [--demo]
//! ```

use donka_app::install::{self, Options, Outcome};
use std::path::PathBuf;

const USAGE: &str = "usage:
  donka-admin init [--dir <dir>] [--public-url <url>] [--admin-email <email>]
                   [--studio-version <tag>] [--runtime-version <tag>] [--demo]
      Prepares an installation in <dir> (the current folder by default): docker-compose.yml
      and .env with random secrets. Existing files are kept, never overwritten.
      --demo also releases the retail-credit pack to staging at the first start, its bureau
      connector answered by a fake bureau (its key in runtime.env).
      Then: docker compose up -d
  donka-admin demo-bureau
      The demo's fake credit bureau, on port 8080 (the demo-bureau service runs it).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("init") => init(&args[1..]),
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
