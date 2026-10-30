//! `donka-admin`: the commands an operator runs on the server (DNK-64, DNK-74). It ships in
//! the Studio image next to `donka-app`.
//!
//! ```text
//! docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
//!   ghcr.io/youmssi/donka-studio:<version> donka-admin init [--demo]
//! ```

use donka_app::install::{self, assistant, Installation, Outcome, Provided};
use donka_app::upgrade;
use donka_app::upgrade::auto::{self, Role, Window};
use donka_licence::Module;
use std::io::IsTerminal as _;
use std::path::PathBuf;

const USAGE: &str = "usage:
  donka-admin init [--dir <dir>] [--public-url <url>] [--admin-email <email>]
                   [--studio-version <tag>] [--runtime-version <tag>] [--registry <host/path>]
                   [--demo] [--offline] [--role production|staging|demo]
                   [--auto-upgrade \"<day> <HH:MM>-<HH:MM>\"] [--yes]
      Prepares an installation in <dir> (the current folder by default). On a terminal it asks
      its questions, in English or French, each with a default (the options set them); --yes,
      or no terminal, takes the defaults. It writes donka.yaml (the answers, no secret), the
      secrets as files in secrets/, and from them .env, runtime.env and docker-compose.yml.
      An existing donka.yaml is kept and nothing changes.
      --demo also releases the retail-credit pack to staging at the first start, its bureau
      connector answered by a fake bureau (its key in secrets/connector_bureau_api_key).
      --registry names a mirror of the images (ghcr.io/youmssi by default).
      --offline: the server has no internet access, Studio never checks for new releases.
      --role says what the installation is (production by default, demo with --demo);
      --auto-upgrade \"Sun 02:00-04:00\" turns on unattended upgrades in that weekly window,
      staging and demo only.
      Then: docker compose up -d
  donka-admin init --config donka.yaml
      Writes .env, runtime.env and docker-compose.yml again from donka.yaml, without
      questions, after a change to it. Secrets in secrets/ are kept, never overwritten.
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
  donka-admin auto-upgrade [--once]
      The auto-upgrade service: in the window DONKA_AUTO_UPGRADE_WINDOW of a staging or demo
      installation, upgrades to the latest release of the same major version, as
      donka-admin upgrade does (verified with cosign.pub, backed up, rolled back on failure,
      reported in upgrades/). --once looks once, now, and exits: 0 nothing to do or upgraded,
      3 refused or a production installation, 4 rolled back, 5 the restore failed too.
  donka-admin doctor
      Checks, with Studio's settings, what Studio depends on: the database, the release
      storage, the mail server, sign-in, the language model, the licence, the packs and the
      services the live decisions' connectors call. Each line says whether it works, and
      under a failing one, what to change. Run it where Studio runs:
        docker compose run --rm --no-deps app donka-admin doctor
      Exit status: 0 everything works, 1 a check failed.
  donka-admin modules [enable|disable <module>...] [--dir <dir>]
      Lists the optional modules (explain, connectors, analysis) and whether they are on, or
      turns them on or off in donka.yaml, within what the licence includes. Their data is kept.
      It takes effect when Studio restarts (docker compose up -d app), which audits the change.
  donka-admin support-bundle [--since <hours>]
      Writes support-<time>.tar.gz in the installation folder, for Donka's support: versions,
      configuration, the doctor's report, the last day of logs (or --since hours) and the
      latest upgrade reports. Secret values, passwords in addresses, tokens, email addresses and
      public IP addresses are removed; read it before sending it. Run it as the upgrade runs:
        docker run --rm -u \"$(id -u):$(id -g)\" --group-add \"$(stat -c %g /var/run/docker.sock)\" \\
          -v /var/run/docker.sock:/var/run/docker.sock -v \"$PWD:/install\" -w /install \\
          ghcr.io/youmssi/donka-studio:<installed version> donka-admin support-bundle
  donka-admin demo-bureau
      The demo's fake credit bureau, on port 8080 (the demo-bureau service runs it).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("init") => init(&args[1..]),
        Some("upgrade") => run_upgrade(&args[1..]),
        Some("auto-upgrade") => auto_upgrade(&args[1..]),
        Some("doctor") if args.len() == 1 => doctor(),
        Some("modules") => modules(&args[1..]),
        Some("support-bundle") => support_bundle(&args[1..]),
        Some("demo-bureau") if args.len() == 1 => demo_bureau(),
        _ => Err(USAGE.to_owned()),
    };
    if let Err(err) = done {
        eprintln!("{err}");
        std::process::exit(2);
    }
}

fn doctor() -> Result<(), String> {
    let key = donka_licence::PublicKey::built_in().map_err(|err| err.to_string())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    let checks = runtime.block_on(donka_app::doctor::run(
        donka_app::config::Config::from_env(),
        key,
    ));
    print!("{}", donka_app::doctor::render(&checks));
    if checks.iter().any(donka_app::doctor::Check::failed) {
        std::process::exit(1);
    }
    Ok(())
}

fn support_bundle(args: &[String]) -> Result<(), String> {
    let mut since_hours = 24;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--since" => {
                since_hours = rest
                    .next()
                    .and_then(|hours| hours.parse().ok())
                    .filter(|hours: &i64| *hours > 0)
                    .ok_or("--since needs a number of hours")?;
            }
            other => return Err(format!("unexpected `{other}`\n\n{USAGE}")),
        }
    }
    let options = donka_app::support::Options {
        dir: PathBuf::from("."),
        since_hours,
    };
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    let path = runtime.block_on(donka_app::support::run(&options))?;
    println!(
        "Written: {}\nRead it before sending it to Donka's support.",
        path.display()
    );
    Ok(())
}

fn modules(args: &[String]) -> Result<(), String> {
    let mut dir = PathBuf::from(".");
    let mut action = None;
    let mut names = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dir" => dir = PathBuf::from(rest.next().ok_or("--dir needs a folder")?),
            "enable" | "disable" if action.is_none() => action = Some(arg.as_str() == "enable"),
            name if action.is_some() => names.push(Module::parse(name).ok_or_else(|| {
                format!("`{name}` is not a module: explain, connectors or analysis")
            })?),
            other => return Err(format!("unexpected `{other}`\n\n{USAGE}")),
        }
    }
    let installation = Installation::read(&dir)?;
    let mut on = installation
        .modules
        .clone()
        .unwrap_or_else(|| Module::ALL.into_iter().collect());
    let Some(enable) = action else {
        for module in Module::ALL {
            let state = if on.contains(&module) { "on" } else { "off" };
            println!("  {state:<3}  {module}");
        }
        println!(
            "Only the modules the licence includes are offered; Settings in Studio says which."
        );
        return Ok(());
    };
    if names.is_empty() {
        return Err("name the modules: explain, connectors or analysis".to_owned());
    }
    for module in &names {
        if enable {
            on.insert(*module);
        } else {
            on.remove(module);
        }
    }
    // Every module on is the default: the entry goes, and a new module will be on too.
    let all = on.len() == Module::ALL.len();
    install::set_modules(&dir, (!all).then_some(&on))?;
    let names: Vec<&str> = names.iter().map(|module| module.as_str()).collect();
    println!(
        "{} {} in donka.yaml. Restart Studio to apply it: docker compose up -d app",
        if enable { "Turned on" } else { "Turned off" },
        names.join(", ")
    );
    Ok(())
}

fn init(args: &[String]) -> Result<(), String> {
    let mut installation = Installation::for_version(donka_app::VERSION);
    let mut dir = PathBuf::from(".");
    let mut config = None;
    let mut ask = std::io::stdin().is_terminal();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--demo" => {
                installation.demo = true;
                installation.role = Role::Demo;
                continue;
            }
            "--offline" => {
                installation.offline = true;
                continue;
            }
            "--yes" => {
                ask = false;
                continue;
            }
            _ => {}
        }
        let value = rest
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n\n{USAGE}"))?
            .clone();
        match flag.as_str() {
            "--dir" => dir = PathBuf::from(value),
            "--config" => config = Some(PathBuf::from(value)),
            "--public-url" => installation.public_url = value,
            "--admin-email" => installation.administrator = value,
            "--studio-version" => installation.images.studio = value,
            "--runtime-version" => installation.images.runtime = value,
            "--registry" => installation.images.registry = Some(value),
            "--role" => installation.role = Role::parse(&value)?,
            "--auto-upgrade" => installation.auto_upgrade = Some(Window::parse(&value)?),
            _ => return Err(format!("unknown option `{flag}`\n\n{USAGE}")),
        }
    }
    let random = |buf: &mut [u8]| {
        getrandom::fill(buf).map_err(|err| format!("no secure random source: {err}"))
    };
    let outcomes = if let Some(config) = config {
        // A replay: the answers are the file's, the secrets those already in secrets/.
        if config.file_name().and_then(|name| name.to_str()) != Some(install::CONFIG) {
            return Err(format!(
                "--config takes the installation's {}",
                install::CONFIG
            ));
        }
        dir = config
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        installation = Installation::read(&dir)?;
        install::apply(&dir, &installation, &Provided::default(), random)?
    } else if dir.join(install::CONFIG).exists() {
        println!(
            "kept     {} (it exists; nothing changed)\n\
             To apply a change to it: donka-admin init --config {}",
            dir.join(install::CONFIG).display(),
            install::CONFIG
        );
        return Ok(());
    } else {
        let provided = if ask {
            let (answers, provided) = assistant::run(&mut assistant::Terminal, &installation)?;
            installation = answers;
            provided
        } else {
            Provided::default()
        };
        install::init(&dir, &installation, &provided, random)?
    };
    for outcome in &outcomes {
        match outcome {
            Outcome::Written(path) => println!("written  {}", path.display()),
            Outcome::Kept(path) => println!("kept     {}", path.display()),
        }
    }
    println!(
        "\nNext:\n  docker compose up -d\n  docker compose logs app | grep setup-password\n\
         Then open {} and choose {}'s password with that link.",
        installation.public_url, installation.administrator
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
    options.credentials = registry_credentials();
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    let report = runtime.block_on(upgrade::run(options))?;
    std::process::exit(exit_code(&report.outcome))
}

fn auto_upgrade(args: &[String]) -> Result<(), String> {
    let once = match args {
        [] => false,
        [flag] if flag == "--once" => true,
        _ => return Err(USAGE.to_owned()),
    };
    let settings = auto::Settings {
        dir: PathBuf::from("."),
        key: PathBuf::from("cosign.pub"),
        credentials: registry_credentials(),
    };
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    if !once {
        tracing_subscriber::fmt().init();
        runtime.block_on(auto::watch(settings));
        return Ok(());
    }
    let now = chrono::Local::now().naive_local();
    let code = match runtime.block_on(auto::tick(&settings, now, &mut None))? {
        auto::Tick::Production => {
            println!("unattended upgrades never run in production (DONKA_INSTALLATION_ROLE)");
            3
        }
        auto::Tick::Closed => {
            println!("outside the window (DONKA_AUTO_UPGRADE_WINDOW): nothing to do");
            0
        }
        auto::Tick::AlreadyTried | auto::Tick::UpToDate => {
            println!("no later release of the same major version: nothing to do");
            0
        }
        auto::Tick::Ran(outcome) => exit_code(&outcome),
    };
    std::process::exit(code)
}

/// The read token for a private registry (Studio's image), from the environment.
fn registry_credentials() -> Option<(String, String)> {
    let variable = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    variable("DONKA_REGISTRY_TOKEN").map(|token| {
        (
            variable("DONKA_REGISTRY_USER").unwrap_or_else(|| "donka".to_owned()),
            token,
        )
    })
}

fn exit_code(outcome: &upgrade::Outcome) -> i32 {
    match outcome {
        upgrade::Outcome::Upgraded => 0,
        upgrade::Outcome::Refused => 3,
        upgrade::Outcome::RolledBack => 4,
        upgrade::Outcome::RestoreFailed => 5,
    }
}

/// Serves the demo's fake bureau until the container stops.
fn demo_bureau() -> Result<(), String> {
    // As the Runtime reads it: the value, or the file that holds it (secrets/ in an installation).
    let key = match std::env::var("DONKA_SECRET_BUREAU_API_KEY_FILE") {
        Ok(file) => install::read_secret(std::path::Path::new(&file))?,
        Err(_) => std::env::var("DONKA_SECRET_BUREAU_API_KEY").unwrap_or_default(),
    };
    if key.trim().is_empty() {
        return Err(
            "DONKA_SECRET_BUREAU_API_KEY(_FILE) is not set: run donka-admin init --demo".to_owned(),
        );
    }
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
