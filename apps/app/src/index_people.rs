//! `donka-app decision-log index-people`: indexes the records stored before people could be
//! found by their identifiers (DNK-75), oldest first. Batches commit one by one, each with its
//! audit events and where it stopped, so the command can be stopped and run again: it carries
//! on with the records left. Records received since are indexed as they arrive.

use donka_decision_log::DecisionLog;
use std::io::Write;

/// Records read per transaction.
const BATCH: i64 = 500;

/// Indexes every record left, writing progress to `out`.
pub async fn run(log: &DecisionLog, out: &mut impl Write) -> Result<u64, String> {
    if log
        .people_index_finished()
        .await
        .map_err(|err| err.to_string())?
    {
        line(out, "Every decision record is already indexed.");
        return Ok(0);
    }
    line(
        out,
        "Indexing the decision records stored before people could be found.",
    );
    let mut done = 0;
    loop {
        let progress = log.index_people_batch(BATCH).await.map_err(|err| {
            format!("stopped after {done} records: {err}. Run the command again to carry on.")
        })?;
        done += progress.records;
        if progress.finished {
            break;
        }
        line(out, &format!("  {done} records"));
    }
    line(out, &format!("Done: {done} records indexed."));
    Ok(done)
}

fn line(out: &mut impl Write, text: &str) {
    // The terminal going away must not stop the indexing halfway.
    let _ = writeln!(out, "{text}");
}
