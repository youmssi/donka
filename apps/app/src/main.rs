use chrono::Duration;
use donka_app::auth::CookieSettings;
use donka_app::{config::Config, router, AppState};
use donka_db::DbOptions;
use donka_engine::ZenRuntime;
use donka_identity::clock::SystemClock;
use donka_identity::{Identity, Policy};
use donka_mail::SmtpMailer;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "warn,donka_app=info,donka_identity=info,tower_http=info".into()
            }),
        )
        .init();

    // One clear line for operators; a bad setting is not a crash worth a backtrace.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => exit_with(&format!("invalid configuration: {err}")),
    };
    let runtime = match config.engine_workers {
        Some(n) => ZenRuntime::new(n),
        None => ZenRuntime::default(),
    };
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
    let identity = Identity::new(db.clone(), Arc::new(SystemClock), policy);

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
    tokio::spawn(donka_app::email_worker::run(
        identity.clone(),
        Arc::new(mailer),
        config.public_url.clone(),
    ));

    if let Some(dir) = &config.web_dir {
        donka_app::web::check_export(dir).unwrap_or_else(|err| exit_with(&err));
    }
    let app = router(
        AppState {
            runtime: Arc::new(runtime),
            db,
            identity,
            cookies,
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
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// Startup failures an operator must fix (configuration, database): one line, no backtrace.
fn exit_with(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
