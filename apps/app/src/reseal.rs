//! `donka-app decision-log reseal`: after a key rotation (DNK-40), re-seals the
//! records sealed with a previous key with the current one, so the previous key
//! can be removed. Batches commit one by one, each with its audit events, so the
//! command can be stopped and run again: it carries on with the records left.

use donka_decision_log::{DecisionLog, KeyUsage};
use std::io::Write;

/// Records re-sealed per transaction.
const BATCH: i64 = 500;

/// Re-seals every record it can, writing progress to `out`. An error names
/// what the operator has to do.
pub async fn run(log: &DecisionLog, out: &mut impl Write) -> Result<u64, String> {
    let usage = log
        .previous_key_usage()
        .await
        .map_err(|err| err.to_string())?;
    let current = log.key_id();
    if usage.iter().all(|key| key.records == 0) {
        line(
            out,
            &format!("Every decision record is sealed with the current key ({current})."),
        );
        unused_keys(out, &usage);
        return Ok(0);
    }
    let missing: Vec<&KeyUsage> = usage.iter().filter(|key| !key.configured).collect();
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|key| format!("{} ({} records)", key.key_id, key.records))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "records are sealed with keys DONKA_DECISION_LOG_PREVIOUS_KEYS does not hold: {list}. \
             Add those keys and run the command again."
        ));
    }

    let total: i64 = usage.iter().map(|key| key.records).sum();
    line(
        out,
        &format!("Re-sealing {total} decision records with the current key ({current})."),
    );
    let mut done = 0;
    loop {
        let resealed = log.reseal_batch(BATCH).await.map_err(|err| {
            format!("stopped after {done} records: {err}. Run the command again to carry on.")
        })?;
        if resealed == 0 {
            break;
        }
        done += resealed;
        line(out, &format!("  {done} of {total}"));
    }

    line(
        out,
        &format!("Done: {done} records re-sealed with the current key ({current})."),
    );
    unused_keys(out, &usage);
    Ok(done)
}

/// Names the previous keys no record needs any more.
fn unused_keys(out: &mut impl Write, usage: &[KeyUsage]) {
    let keys: Vec<&str> = usage.iter().map(|key| key.key_id.as_str()).collect();
    if !keys.is_empty() {
        line(
            out,
            &format!(
                "No record uses {} any more: remove it from DONKA_DECISION_LOG_PREVIOUS_KEYS \
                 and restart Studio.",
                keys.join(", ")
            ),
        );
    }
}

fn line(out: &mut impl Write, text: &str) {
    // The terminal going away must not stop the re-seal halfway.
    let _ = writeln!(out, "{text}");
}
