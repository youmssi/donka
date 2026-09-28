use donka_app::{config::Config, router, AppState};
use donka_db::DbOptions;
use donka_engine::ZenRuntime;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "donka_app=info,tower_http=info".into()),
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

    let app = router(
        AppState {
            runtime: Arc::new(runtime),
            db,
        },
        &config.api_base_path,
    );

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(
        "Donka Studio app listening on {} under {}",
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
