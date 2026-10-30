//! `donka-admin load-test` (DNK-67): calls one decision of a Runtime and measures its answers.
//!
//! - **At a rate** (`--rate`): requests leave on a fixed schedule whatever the answers, and
//!   each is timed from when it was due, so a slow Runtime cannot hide its own queueing
//!   (coordinated omission). This is the latency at a given load.
//! - **As fast as it answers** (`--concurrency`): that many callers each send the next request
//!   when the previous one is answered. This is the maximum throughput.
//!
//! The inputs are taken in turn from a JSON array; requests in the warm-up are not counted.

use reqwest::header::{HeaderValue, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

/// How long one request may take before it counts as an error.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub enum Load {
    /// Requests per second, sent on schedule.
    Rate(u32),
    /// Callers each waiting for their answer before the next request.
    Concurrency(usize),
}

pub struct Options {
    /// The decision's evaluate address on the Runtime.
    pub url: String,
    /// The Runtime token; `None` for a Runtime that needs none.
    pub token: Option<String>,
    /// The decision's inputs, sent in turn as `context`.
    pub inputs: Vec<Value>,
    pub load: Load,
    pub duration: Duration,
    pub warmup: Duration,
}

/// What was measured, in milliseconds and requests per second.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Requests sent, the warm-up's included, and those answered with a decision: what the
    /// decision log should then hold.
    pub sent: usize,
    pub answered: usize,
    /// Requests measured (after the warm-up), and those not answered with a decision.
    pub requests: usize,
    pub errors: usize,
    pub throughput: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// One measured request: how long it took, and whether it was answered with a decision.
struct Sample {
    latency: Duration,
    ok: bool,
    counted: bool,
}

pub async fn run(options: Options) -> Result<Report, String> {
    if options.inputs.is_empty() {
        return Err("no inputs: give a JSON array of at least one input".to_owned());
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .pool_max_idle_per_host(1024)
        .build()
        .map_err(|err| err.to_string())?;
    let target = Arc::new(Target {
        client,
        url: options.url,
        token: options
            .token
            .map(|token| {
                HeaderValue::from_str(&token).map_err(|_| "the token is not a header value")
            })
            .transpose()?,
        bodies: options
            .inputs
            .into_iter()
            .map(|input| json!({ "context": input }).to_string())
            .collect(),
    });
    let (tx, mut rx) = mpsc::unbounded_channel::<Sample>();
    let started = Instant::now();
    let counted_from = started + options.warmup;
    let end = counted_from + options.duration;
    match options.load {
        Load::Rate(rate) => {
            if rate == 0 {
                return Err("--rate must be at least 1".to_owned());
            }
            let mut ticks = tokio::time::interval(Duration::from_secs(1) / rate);
            // A tick missed while the machine was busy still sends its request, late: that
            // lateness is part of the latency measured.
            ticks.set_missed_tick_behavior(MissedTickBehavior::Burst);
            let mut sent = 0usize;
            loop {
                let due = ticks.tick().await.into_std();
                if due >= end {
                    break;
                }
                let (target, tx) = (target.clone(), tx.clone());
                let index = sent;
                sent += 1;
                tokio::spawn(async move {
                    let ok = target.call(index).await;
                    let _ = tx.send(Sample {
                        latency: due.elapsed(),
                        ok,
                        counted: due >= counted_from,
                    });
                });
            }
        }
        Load::Concurrency(callers) => {
            if callers == 0 {
                return Err("--concurrency must be at least 1".to_owned());
            }
            for caller in 0..callers {
                let (target, tx) = (target.clone(), tx.clone());
                tokio::spawn(async move {
                    let mut index = caller;
                    loop {
                        let due = Instant::now();
                        if due >= end {
                            break;
                        }
                        let ok = target.call(index).await;
                        let _ = tx.send(Sample {
                            latency: due.elapsed(),
                            ok,
                            counted: due >= counted_from,
                        });
                        index += callers;
                    }
                });
            }
        }
    }
    drop(tx);
    let mut latencies = Vec::new();
    let (mut errors, mut sent, mut answered) = (0, 0, 0);
    while let Some(sample) = rx.recv().await {
        sent += 1;
        answered += usize::from(sample.ok);
        if sample.counted {
            latencies.push(sample.latency);
            if !sample.ok {
                errors += 1;
            }
        }
    }
    Ok(Report {
        sent,
        answered,
        ..report(latencies, errors, options.duration)
    })
}

struct Target {
    client: reqwest::Client,
    url: String,
    token: Option<HeaderValue>,
    bodies: Vec<String>,
}

impl Target {
    /// Sends the `index`th request; true when the Runtime answered with a decision.
    async fn call(&self, index: usize) -> bool {
        let body = &self.bodies[index % self.bodies.len()];
        let mut request = self
            .client
            .post(&self.url)
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone());
        if let Some(token) = &self.token {
            request = request.header("x-access-token", token.clone());
        }
        match request.send().await {
            Ok(response) => {
                let ok = response.status().is_success();
                // Read the answer: the request is done only when the decision has arrived.
                ok && response.bytes().await.is_ok()
            }
            Err(_) => false,
        }
    }
}

/// The percentiles of `latencies` (nearest rank) and the throughput over `duration`.
fn report(mut latencies: Vec<Duration>, errors: usize, duration: Duration) -> Report {
    latencies.sort_unstable();
    let at = |percentile: f64| {
        if latencies.is_empty() {
            return 0.0;
        }
        let rank = (percentile / 100.0 * latencies.len() as f64).ceil() as usize;
        millis(latencies[rank.clamp(1, latencies.len()) - 1])
    };
    Report {
        sent: latencies.len(),
        answered: latencies.len() - errors,
        requests: latencies.len(),
        errors,
        throughput: round(latencies.len() as f64 / duration.as_secs_f64()),
        p50_ms: at(50.0),
        p95_ms: at(95.0),
        p99_ms: at(99.0),
        max_ms: latencies.last().copied().map(millis).unwrap_or(0.0),
    }
}

fn millis(duration: Duration) -> f64 {
    round(duration.as_secs_f64() * 1000.0)
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests;
