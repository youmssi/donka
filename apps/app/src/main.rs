use chrono::Duration;
use donka_app::auth::CookieSettings;
use donka_app::{config::Config, router, AppState};
use donka_audit::AuditLog;
use donka_db::DbOptions;
use donka_decision::Decisions;
use donka_decision_log::{Cipher, DecisionLog};
use donka_engine::{DecisionRuntime, ZenRuntime};
use donka_identity::{Identity, Policy};
use donka_mail::SmtpMailer;
use donka_project::Projects;
use donka_release::{ReleaseSettings, Releases};
use donka_shared::clock::SystemClock;
use donka_storage::ObjectStorage;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // No argument runs Studio; `decision-log reseal` re-seals the decision log
    // after a key rotation and exits.
    let command: Vec<String> = std::env::args().skip(1).collect();
    let reseal = match command.as_slice() {
        [] => false,
        [group, action] if group == "decision-log" && action == "reseal" => true,
        _ => exit_with(&format!(
            "unknown command `{}`. Commands: decision-log reseal (re-seal decision records \
             with the current key after a rotation); none to run Studio.",
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
    let identity = Identity::new(db.clone(), clock.clone(), policy);
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
    let decision_log = DecisionLog::new(
        db.clone(),
        clock,
        Arc::new(cipher),
        releases.clone(),
        runtime.clone(),
        Duration::days(config.decision_log_retention_days.into()),
    );

    if reseal {
        match donka_app::reseal::run(&decision_log, &mut std::io::stderr()).await {
            Ok(_) => return Ok(()),
            Err(err) => exit_with(&err),
        }
    }
    warn_about_previous_keys(&decision_log).await;

    if let Some(email) = &config.bootstrap_admin_email {
        match identity.bootstrap_admin(email, config.default_locale).await {
            Ok(Some(token)) => {
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

    let mailer = SmtpMailer::new(&config.smtp_url, &config.smtp_from)
        .unwrap_or_else(|err| exit_with(&format!("DONKA_SMTP_URL / DONKA_SMTP_FROM: {err}")));
    tokio::spawn(donka_app::publish_worker::run(releases.clone()));
    tokio::spawn(donka_app::purge_worker::run(decision_log.clone()));
    tokio::spawn(donka_app::email_worker::run(
        identity.clone(),
        releases.clone(),
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
            cookies,
            auth_limits: donka_app::rate_limit::AuthLimits::new(
                config.sign_in_rate_limit,
                config.password_reset_rate_limit,
                config.trusted_proxies.clone(),
            ),
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
