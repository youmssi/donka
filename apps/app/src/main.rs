use donka_app::{config::Config, router, AppState};
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

    let config = Config::from_env()?;
    let runtime = match config.engine_workers {
        Some(n) => ZenRuntime::new(n),
        None => ZenRuntime::default(),
    };
    let app = router(AppState {
        runtime: Arc::new(runtime),
    });

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!("Donka Studio app listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
