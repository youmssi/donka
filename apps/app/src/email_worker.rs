//! Background delivery of the emails queued by the identity module (account
//! emails), the release module (approval requests) and the analysis module
//! (population stability alerts, DNK-57).

use donka_analysis::Analysis;
use donka_identity::Identity;
use donka_installation::Installation;
use donka_mail::Mailer;
use donka_release::Releases;
use std::sync::Arc;
use std::time::Duration;

/// Picks up retries that come due and emails queued by another instance.
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Runs until the process stops. An email interrupted by shutdown is rolled
/// back and sent by the next run.
pub async fn run(
    identity: Identity,
    releases: Releases,
    analysis: Analysis,
    installation: Installation,
    mailer: Arc<dyn Mailer>,
    public_url: String,
) {
    loop {
        // Read each pass: administrators may rename the organisation at any time.
        let organisation = installation.organisation().await.unwrap_or_else(|err| {
            tracing::warn!(%err, "cannot read the organisation's name; emails go unsigned");
            None
        });
        let organisation = organisation.as_deref();
        if let Err(err) = identity
            .deliver_due_emails(mailer.as_ref(), &public_url, organisation)
            .await
        {
            tracing::warn!(%err, "account email delivery pass failed");
        }
        if let Err(err) = releases
            .deliver_due_emails(mailer.as_ref(), organisation)
            .await
        {
            tracing::warn!(%err, "approval email delivery pass failed");
        }
        if let Err(err) = analysis.deliver_due_alerts(mailer.as_ref(), organisation).await {
            tracing::warn!(%err, "stability alert delivery pass failed");
        }
        tokio::select! {
            () = identity.email_queued() => {}
            () = releases.email_queued() => {}
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}
