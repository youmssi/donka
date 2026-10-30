use chrono::Duration;
use donka_app::auth::CookieSettings;
use donka_app::{config::Config, router, AppState};
use donka_audit::AuditLog;
use donka_db::DbOptions;
use donka_decision::Decisions;
use donka_decision_log::{Cipher, DecisionLog, PersonKey};
use donka_engine::{DecisionRuntime, ZenRuntime};
use donka_identity::{Identity, Policy};
use donka_mail::SmtpMailer;
use donka_pack::{Catalogue, Packs};
use donka_project::Projects;
use donka_release::{ReleaseSettings, Releases};
use donka_shared::clock::SystemClock;
use donka_storage::ObjectStorage;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // No argument runs Studio; `decision-log reseal` re-seals the decision log
    // after a key rotation, `decision-log index-people` indexes the records stored
    // before people could be found by their identifiers; both exit when done.
    let command: Vec<String> = std::env::args().skip(1).collect();
    let task = match command.as_slice() {
        [] => None,
        [group, action] if group == "decision-log" && action == "reseal" => Some(Task::Reseal),
        [group, action] if group == "decision-log" && action == "index-people" => {
            Some(Task::IndexPeople)
        }
        _ => exit_with(&format!(
            "unknown command `{}`. Commands: decision-log reseal (re-seal decision records \
             with the current key after a rotation); decision-log index-people (index the \
             records stored before people could be found by their identifiers); none to run \
             Studio.",
            command.join(" ")
        )),
    };
    // One clear line for operators; a bad setting is not a crash worth a backtrace.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => exit_with(&format!("invalid configuration: {err}")),
    };
    let telemetry = donka_app::telemetry::init(config.otel_enabled)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_OTEL_ENABLED: {err}")));
    if config.otel_enabled {
        tracing::info!("traces and request metrics are exported over OTLP");
    }
    let runtime: Arc<dyn DecisionRuntime> = Arc::new(match config.engine_workers {
        Some(n) => ZenRuntime::new(n),
        None => ZenRuntime::default(),
    });
    let db_options = DbOptions {
        max_connections: config.db_max_connections,
        ..DbOptions::default()
    };
    let db = donka_db::connect(&config.database_url, &db_options)
        .await
        .unwrap_or_else(|err| exit_with(&err.to_string()));
    if config.db_migrate {
        donka_db::migrate(&db)
            .await
            .unwrap_or_else(|err| exit_with(&err.to_string()));
        tracing::info!("database migrations are up to date");
    }

    let policy = Policy {
        session_idle_timeout: Duration::minutes(config.session_idle_minutes.into()),
        max_failed_sign_ins: config.sign_in_max_failures,
        lock_duration: Duration::minutes(config.sign_in_lock_minutes.into()),
        invitation_link_lifetime: Duration::hours(config.invitation_link_hours.into()),
        reset_link_lifetime: Duration::minutes(config.password_reset_link_minutes.into()),
        email_max_attempts: config.email_max_attempts,
        ..Policy::default()
    };
    let link_hours = config.invitation_link_hours;
    let cookies = CookieSettings {
        secure: config.cookie_secure,
        max_age_seconds: policy.session_idle_timeout.num_seconds(),
    };
    let clock = Arc::new(SystemClock);
    let second_step_keys = Cipher::from_base64(&config.second_step_key)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_SECOND_STEP_KEY {err}")))
        .with_previous(config.second_step_previous_keys.iter().map(String::as_str))
        .unwrap_or_else(|err| exit_with(&format!("DONKA_SECOND_STEP_PREVIOUS_KEYS {err}")));
    let identity = Identity::new(
        db.clone(),
        clock.clone(),
        policy,
        Arc::new(second_step_keys),
    );
    let projects = Projects::new(db.clone(), clock.clone());
    let decisions = Decisions::new(db.clone(), clock.clone(), runtime.clone());
    let storage = ObjectStorage::from_url(&config.storage_url, config.storage_options.clone())
        .unwrap_or_else(|err| exit_with(&format!("DONKA_STORAGE_URL: {err}")));
    let releases = Releases::new(
        db.clone(),
        clock.clone(),
        decisions.clone(),
        projects.clone(),
        Arc::new(storage),
        ReleaseSettings {
            publish_max_attempts: config.publish_max_attempts,
            email_max_attempts: config.email_max_attempts,
            public_url: config.public_url.clone(),
        },
    );
    let audit = AuditLog::new(db.clone());
    let cipher = Cipher::from_base64(&config.decision_log_key)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_DECISION_LOG_KEY {err}")))
        .with_previous(config.decision_log_previous_keys.iter().map(String::as_str))
        .unwrap_or_else(|err| exit_with(&format!("DONKA_DECISION_LOG_PREVIOUS_KEYS {err}")));
    let person_key = PersonKey::from_base64(&config.person_index_key)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_PERSON_INDEX_KEY {err}")));
    let decision_log = DecisionLog::new(
        db.clone(),
        clock.clone(),
        Arc::new(cipher),
        Arc::new(person_key),
        releases.clone(),
        runtime.clone(),
        Duration::days(config.decision_log_retention_days.into()),
    );

    if let Some(task) = task {
        let done = match task {
            Task::Reseal => donka_app::reseal::run(&decision_log, &mut std::io::stderr()).await,
            Task::IndexPeople => {
                donka_app::index_people::run(&decision_log, &mut std::io::stderr()).await
            }
        };
        match done {
            Ok(_) => return Ok(()),
            Err(err) => exit_with(&err),
        }
    }
    warn_about_previous_keys(&decision_log).await;
    match decision_log.people_index_finished().await {
        Ok(true) => {}
        Ok(false) => tracing::warn!(
            "decision records stored before people could be found by their identifiers are not \
             indexed yet: run `donka-app decision-log index-people`"
        ),
        Err(err) => tracing::warn!(%err, "cannot check whether the people index is complete"),
    }

    let mut first_start = false;
    if let Some(email) = &config.bootstrap_admin_email {
        match identity.bootstrap_admin(email, config.default_locale).await {
            Ok(Some(token)) => {
                first_start = true;
                // Printed once, to the operator's console, as the only way into a new
                // installation. The link works once and expires with the invitation lifetime.
                eprintln!(
                    "\nFirst administrator created: {email}\n\
                     Set the password within {link_hours} hours (the link works once):\n  \
                     {}\n",
                    donka_identity::setup_link(&config.public_url, config.default_locale, &token)
                );
            }
            Ok(None) => tracing::info!("users already exist; DONKA_BOOTSTRAP_ADMIN_EMAIL ignored"),
            Err(err) => exit_with(&format!("cannot create the first administrator: {err}")),
        }
    }
    if let Some(file) = &config.runtime_log_token_file {
        let wired = donka_app::wiring::runtime_log_token(
            &identity,
            &decision_log,
            file,
            config.bootstrap_admin_email.as_deref(),
        )
        .await;
        match wired {
            Ok(donka_app::wiring::Wired::Issued) => tracing::info!(
                file = %file.display(),
                "the staging Runtime's decision-log token is issued and written"
            ),
            Ok(donka_app::wiring::Wired::Already) => {}
            Err(err) => tracing::error!(
                %err,
                "cannot wire the staging Runtime to the decision log (DONKA_RUNTIME_LOG_TOKEN_FILE)"
            ),
        }
    }

    let mailer = SmtpMailer::new(&config.smtp_url, &config.smtp_from)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_SMTP_URL / DONKA_SMTP_FROM: {err}")));
    tokio::spawn(donka_app::publish_worker::run(releases.clone()));
    tokio::spawn(donka_app::purge_worker::run(decision_log.clone()));
    let installation = donka_installation::Installation::new(db.clone(), clock.clone());
    tokio::spawn(donka_app::email_worker::run(
        identity.clone(),
        releases.clone(),
        installation.clone(),
        Arc::new(mailer),
        config.public_url.clone(),
    ));

    let explainer = config.explain.clone().map(|settings| {
        tracing::info!(url = %settings.url, model = %settings.model, "decisions can be explained");
        donka_explain::Explainer::new(settings)
            .unwrap_or_else(|err| exit_with(&format!("DONKA_EXPLAIN_URL {err}")))
    });

    if let Some(dir) = &config.web_dir {
        donka_app::web::check_export(dir).unwrap_or_else(|err| exit_with(&err));
    }
    let catalogue = match &config.packs_dir {
        Some(dir) => {
            Catalogue::load(dir).unwrap_or_else(|err| exit_with(&format!("DONKA_PACKS_DIR {err}")))
        }
        None => Catalogue::default(),
    };
    tracing::info!(packs = catalogue.packs().len(), "pack catalogue loaded");
    let licence_key = donka_licence::PublicKey::built_in()
        .unwrap_or_else(|err| exit_with(&format!("built-in licence key: {err}")));
    if donka_licence::DEVELOPMENT_BUILD {
        tracing::warn!(
            key = licence_key.id(),
            "this Studio trusts the development licence key: never deliver this build"
        );
    } else if licence_key.is_placeholder() {
        tracing::warn!(
            "this Studio's licence key is a placeholder: no licence verifies until Donka's \
             public key is built in"
        );
    }
    let licensing =
        donka_app::licence::Licensing::new(config.licence_file.clone(), licence_key, clock.clone());
    tokio::spawn(donka_app::licence::watch(licensing.clone()));
    let updates =
        donka_app::updates::Updates::new(donka_app::VERSION, config.update_check_url.clone());
    tokio::spawn(donka_app::updates::watch(updates.clone()));
    let packs = Packs::new(
        db.clone(),
        clock.clone(),
        projects.clone(),
        decisions.clone(),
        decision_log.clone(),
        releases.clone(),
    );
    if config.demo && first_start {
        let set_up = donka_app::demo::set_up(
            &identity,
            &projects,
            &packs,
            &releases,
            &catalogue,
            config.bootstrap_admin_email.as_deref().unwrap_or_default(),
        )
        .await;
        match set_up {
            Ok(project) => tracing::info!(
                %project,
                "demo: the retail-credit pack is imported, released and deployed to staging"
            ),
            Err(err) => tracing::error!(%err, "demo: cannot set up the demo project"),
        }
    }
    let app = router(
        AppState {
            runtime,
            db,
            identity,
            projects,
            decisions,
            releases,
            decision_log,
            explainer,
            audit,
            packs,
            catalogue: Arc::new(catalogue),
            cookies,
            mfa_required: config.mfa_required,
            auth_limits: donka_app::rate_limit::AuthLimits::new(
                config.sign_in_rate_limit,
                config.password_reset_rate_limit,
                config.trusted_proxies.clone(),
            ),
            licensing,
            updates,
            installation,
        },
        &config.api_base_path,
        config.web_dir.as_deref(),
    );

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(
        "Donka Studio {} listening on {} under {}",
        donka_app::VERSION,
        listener.local_addr()?,
        config.api_base_path
    );
    // The peer address feeds the sign-in and password-reset limits (rate_limit.rs).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    telemetry.shutdown();
    Ok(())
}

/// Ctrl-C, or SIGTERM from a container runtime: stop taking requests, finish those in flight.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

/// A maintenance command run instead of Studio.
enum Task {
    Reseal,
    IndexPeople,
}

/// Startup failures an operator must fix (configuration, database): one line, no backtrace.
fn exit_with(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

/// After a rotation: a record sealed with a key Studio no longer holds cannot be
/// read, and a previous key no record uses any more can go.
async fn warn_about_previous_keys(decision_log: &DecisionLog) {
    let usage = match decision_log.previous_key_usage().await {
        Ok(usage) => usage,
        Err(err) => return tracing::warn!(%err, "cannot check which keys seal decision records"),
    };
    for key in usage.iter().filter(|key| !key.configured) {
        tracing::warn!(
            key_id = %key.key_id,
            records = key.records,
            "decision records are sealed with a key that is not configured: they cannot be read \
             until it is in DONKA_DECISION_LOG_PREVIOUS_KEYS"
        );
    }
    for key in usage.iter().filter(|key| key.configured) {
        if key.records > 0 {
            tracing::info!(
                key_id = %key.key_id,
                records = key.records,
                "decision records are sealed with a previous key: run `donka-app decision-log reseal`"
            );
        } else {
            tracing::info!(
                key_id = %key.key_id,
                "no decision record uses this previous key: remove it from DONKA_DECISION_LOG_PREVIOUS_KEYS"
            );
        }
    }
}
