//! Background delivery of the account emails queued by the identity module.

use donka_identity::Identity;
use donka_mail::Mailer;
use std::sync::Arc;
use std::time::Duration;

/// Picks up retries that come due and emails queued by another instance.
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Runs until the process stops. An email interrupted by shutdown is rolled
/// back and sent by the next run.
pub async fn run(identity: Identity, mailer: Arc<dyn Mailer>, public_url: String) {
    loop {
        if let Err(err) = identity
            .deliver_due_emails(mailer.as_ref(), &public_url)
            .await
        {
            tracing::warn!(%err, "email delivery pass failed");
        }
        tokio::select! {
            () = identity.email_queued() => {}
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}
