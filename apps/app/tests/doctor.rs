//! `donka-admin doctor` (DNK-74) reaches the services the live decisions' connectors call.
#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use donka_app::demo;
use donka_app::doctor::{self, Outcome};
use donka_db::PgPool;
use support::*;

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn nothing_live_means_no_connector_to_reach(db: PgPool) {
    let checks = doctor::connectors(&db, None).await;
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
        doctor::connectors(&db, None).await[0].outcome,
        Outcome::Off(_)
    ));

    app.releases.publish_due().await.unwrap();
    let checks = doctor::connectors(&db, None).await;
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

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_live_connector_the_runtime_would_refuse_names_the_setting_to_add(db: PgPool) {
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
    app.releases.publish_due().await.unwrap();

    // The demo bureau may be called, but its key is listed for no host.
    let destinations = donka_engine::Destinations {
        hosts: vec!["demo-bureau".to_owned()],
        secrets: Default::default(),
    };
    let checks = doctor::connectors(&db, Some(&destinations)).await;
    let refused = checks
        .iter()
        .find_map(|check| match &check.outcome {
            Outcome::Failing { reason, fix } if reason.starts_with("secret ") => {
                Some((reason, fix))
            }
            _ => None,
        })
        .expect("the refused connector is reported");
    assert!(
        refused
            .0
            .starts_with("secret BUREAU_API_KEY may not be sent to demo-bureau"),
        "{}",
        refused.0
    );
    assert!(
        refused
            .0
            .contains("retail-credit/credit-application on staging"),
        "{}",
        refused.0
    );
    assert_eq!(
        refused.1,
        "add demo-bureau to connector_hosts.BUREAU_API_KEY in donka.yaml, then donka-admin init \
         --config donka.yaml"
    );

    // Allowed for its key too: nothing is refused.
    let allowed = donka_engine::Destinations {
        hosts: vec!["demo-bureau".to_owned()],
        secrets: [("BUREAU_API_KEY".to_owned(), vec!["demo-bureau".to_owned()])].into(),
    };
    let checks = doctor::connectors(&db, Some(&allowed)).await;
    assert!(
        checks.iter().all(|check| !matches!(
            &check.outcome,
            Outcome::Failing { reason, .. } if reason.starts_with("secret ")
        )),
        "{checks:?}"
    );
}
