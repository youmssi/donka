//! Background publishing of the release artifacts queued by deployments.

use donka_release::Releases;
use std::time::Duration;

/// Picks up retries that come due and deployments queued by another instance.
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Runs until the process stops. A deployment interrupted by shutdown is
/// rolled back and published by the next run.
pub async fn run(releases: Releases) {
    loop {
        if let Err(err) = releases.publish_due().await {
            tracing::warn!(%err, "publishing pass failed");
        }
        tokio::select! {
            () = releases.deployment_queued() => {}
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}
