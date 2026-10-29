//! `donka-admin doctor` (DNK-74) reaches the services the live decisions' connectors call.
#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use donka_app::demo;
use donka_app::doctor::{self, Outcome};
use donka_db::PgPool;
use support::*;

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn nothing_live_means_no_connector_to_reach(db: PgPool) {
    let checks = doctor::connectors(&db).await;
    assert_eq!(checks.len(), 1);
    assert!(matches!(checks[0].outcome, Outcome::Off(_)), "{checks:?}");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_live_connector_that_cannot_be_reached_names_its_decision(db: PgPool) {
    let app = with_database(db.clone());
    signed_in_admin(&app).await;
    demo::set_up(
        &app.identity,
        &app.projects,
        &app.packs,
        &app.releases,
        &catalogue(),
        ADMIN_EMAIL,
    )
    .await
    .unwrap();
    // Asked for, not yet published: not live, so not checked.
    assert!(matches!(
        doctor::connectors(&db).await[0].outcome,
        Outcome::Off(_)
    ));

    app.releases.publish_due().await.unwrap();
    let checks = doctor::connectors(&db).await;
    assert_eq!(checks.len(), 1, "{checks:?}");
    // The demo's fake bureau only resolves inside the installation's network.
    let Outcome::Failing { reason, fix } = &checks[0].outcome else {
        panic!("{checks:?}");
    };
    assert!(
        reason.starts_with("http://demo-bureau:8080/report"),
        "{reason}"
    );
    assert!(
        fix.contains("retail-credit/credit-application on staging"),
        "{fix}"
    );
}
