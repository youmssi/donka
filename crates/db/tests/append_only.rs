#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! Integration tests: need DATABASE_URL pointing at PostgreSQL 16 (see AGENTS.md §5).
//! `sqlx::test` creates a fresh database per test and applies the migrations.

use donka_db::PgPool;

async fn guarded_table(pool: &PgPool) {
    for sql in [
        "CREATE TABLE ledger (id int PRIMARY KEY, note text NOT NULL)",
        "CREATE TRIGGER ledger_append_only BEFORE UPDATE OR DELETE ON ledger \
         FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation()",
        "CREATE TRIGGER ledger_no_truncate BEFORE TRUNCATE ON ledger \
         FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation()",
        "INSERT INTO ledger VALUES (1, 'written once')",
    ] {
        sqlx::query(sql).execute(pool).await.unwrap();
    }
}

fn is_append_only_error(err: sqlx::Error) -> bool {
    let db = err.as_database_error().expect("a database error");
    db.code().as_deref() == Some("42501") && db.message().contains("append-only")
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn inserts_are_allowed(pool: PgPool) {
    guarded_table(&pool).await;
    sqlx::query("INSERT INTO ledger VALUES (2, 'another')")
        .execute(&pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn updates_are_rejected(pool: PgPool) {
    guarded_table(&pool).await;
    let err = sqlx::query("UPDATE ledger SET note = 'changed'")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(is_append_only_error(err));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn deletes_are_rejected(pool: PgPool) {
    guarded_table(&pool).await;
    let err = sqlx::query("DELETE FROM ledger")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(is_append_only_error(err));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn truncate_is_rejected(pool: PgPool) {
    guarded_table(&pool).await;
    let err = sqlx::query("TRUNCATE ledger")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(is_append_only_error(err));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn ready_when_the_database_answers(pool: PgPool) {
    assert!(donka_db::is_ready(&pool, std::time::Duration::from_secs(2)).await);
}

#[tokio::test]
async fn not_ready_when_the_database_is_unreachable() {
    // Port 1 on localhost refuses connections immediately.
    let options = donka_db::DbOptions {
        acquire_timeout: std::time::Duration::from_millis(500),
        ..Default::default()
    };
    let pool = donka_db::connect_lazy("postgres://nobody@127.0.0.1:1/none", &options).unwrap();
    assert!(!donka_db::is_ready(&pool, std::time::Duration::from_secs(2)).await);
}

#[sqlx::test(migrations = false)]
async fn migrations_apply_twice_without_error(pool: PgPool) {
    donka_db::migrate(&pool).await.unwrap();
    donka_db::migrate(&pool).await.unwrap();
}
