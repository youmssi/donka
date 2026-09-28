//! Shared persistence for Studio: the connection pool, the migrations and a
//! readiness probe. Business modules own their tables and queries; this crate
//! only owns what every module needs to reach PostgreSQL.

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub use sqlx::PgPool;

/// Every migration of Studio, embedded at build time from `/migrations`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone)]
pub struct DbOptions {
    pub max_connections: u32,
    /// How long a request waits for a free connection before failing.
    pub acquire_timeout: Duration,
}

impl Default for DbOptions {
    fn default() -> Self {
        Self {
            max_connections: 10,
            acquire_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("cannot connect to PostgreSQL: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
}

/// Connects eagerly so a wrong `DATABASE_URL` fails at startup, not on the first request.
pub async fn connect(url: &str, options: &DbOptions) -> Result<PgPool, DbError> {
    pool_options(options)
        .connect(url)
        .await
        .map_err(DbError::Connect)
}

/// A pool that connects on first use. For tests and tools that may never touch the database.
pub fn connect_lazy(url: &str, options: &DbOptions) -> Result<PgPool, DbError> {
    pool_options(options)
        .connect_lazy(url)
        .map_err(DbError::Connect)
}

/// Applies pending migrations. Safe to run on every start: applied ones are skipped.
pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    MIGRATOR.run(pool).await.map_err(DbError::Migrate)
}

/// True when the database answers within `timeout`.
pub async fn is_ready(pool: &PgPool, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, sqlx::query("SELECT 1").execute(pool)).await,
        Ok(Ok(_))
    )
}

fn pool_options(options: &DbOptions) -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(options.max_connections)
        .acquire_timeout(options.acquire_timeout)
}
