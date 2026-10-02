//! Background delivery of the emails queued by the identity module (account
//! emails) and the release module (approval requests).

use donka_identity::Identity;
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
    mailer: Arc<dyn Mailer>,
    public_url: String,
) {
    loop {
        if let Err(err) = identity
            .deliver_due_emails(mailer.as_ref(), &public_url)
            .await
        {
            tracing::warn!(%err, "account email delivery pass failed");
        }
        if let Err(err) = releases.deliver_due_emails(mailer.as_ref()).await {
            tracing::warn!(%err, "approval email delivery pass failed");
        }
        tokio::select! {
            () = identity.email_queued() => {}
            () = releases.email_queued() => {}
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}
