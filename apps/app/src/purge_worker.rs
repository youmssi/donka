//! Background purge of decision records older than the retention period, and the monthly
//! partitions the coming records go to (DNK-68).

use donka_decision_log::DecisionLog;
use std::time::Duration;

/// Records go within an hour of reaching the end of their retention period.
const INTERVAL: Duration = Duration::from_secs(3600);

/// Runs until the process stops. Each pass makes sure the coming months have their partition,
/// then purges, audited per project it purged.
pub async fn run(decision_log: DecisionLog) {
    loop {
        if let Err(err) = decision_log.ensure_partitions().await {
            tracing::warn!(%err, "decision log partitions could not be created ahead");
        }
        match decision_log.purge().await {
            Ok(0) => {}
            Ok(purged) => tracing::info!(purged, "decision records past retention purged"),
            Err(err) => tracing::warn!(%err, "decision log purge failed"),
        }
        tokio::time::sleep(INTERVAL).await;
    }
}
