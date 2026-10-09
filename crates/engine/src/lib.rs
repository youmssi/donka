//! The only crate in Donka that talks to `zen_engine`.
//!
//! Everything else depends on [`DecisionRuntime`], so the engine can be upgraded
//! or replaced without touching route handlers, and tests can use a fake.
//!
//! A [`Bundle`] is the unit Donka evaluates: every decision of a project at one
//! point in time (a draft, or a release). Graphs reference sub-decisions by key,
//! so evaluating a single graph without its siblings would be wrong.
//!
//! Connector nodes (`donka.connector`) answer with the mock response their
//! author defined, or, when replaying a logged decision, with what they
//! answered at the time: no call to an outside service ever leaves Studio.
//! The Runtime runs the same handler for real.

pub mod contract;

use async_trait::async_trait;
use donka_connectors::{recorded_outputs, ConnectorAdapter};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread::available_parallelism;
use tokio_util::task::LocalPoolHandle;
use zen_engine::loader::MemoryLoader;
use zen_engine::model::DecisionContent;
use zen_engine::{DecisionEngine, EvaluationSerializedOptions, EvaluationTraceKind};

/// Graph nesting limit, the same default the engine uses.
const MAX_DEPTH: u8 = 10;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// A decision in the bundle is not valid JDM; `key` names which one.
    #[error("decision '{key}' is not a valid decision model: {message}")]
    InvalidContent { key: String, message: String },
    #[error("decision '{0}' does not exist in this bundle")]
    NotFound(String),
    /// The engine ran and rejected the input. `details` is the engine's own
    /// error document (node id, message, and trace when requested).
    #[error("evaluation failed")]
    Evaluation { details: Value },
    #[error("engine worker failed: {0}")]
    Internal(String),
}

/// Every decision of a project, keyed the way graphs reference each other
/// (e.g. `person-score`, `bureau/normalize`).
#[derive(Debug, Clone)]
pub struct Bundle {
    keys: Vec<String>,
    loader: Arc<MemoryLoader>,
}

impl Bundle {
    /// Parses every decision up front so a broken model fails when the bundle
    /// is built (on save or release), not on the first applicant.
    pub fn from_json(decisions: BTreeMap<String, Value>) -> Result<Self, RuntimeError> {
        let loader = MemoryLoader::default();
        let mut keys = Vec::with_capacity(decisions.len());
        for (key, raw) in decisions {
            let content: DecisionContent =
                serde_json::from_value(raw).map_err(|err| RuntimeError::InvalidContent {
                    key: key.clone(),
                    message: err.to_string(),
                })?;
            loader.add(key.clone(), content);
            keys.push(key);
        }
        Ok(Self {
            keys,
            loader: Arc::new(loader),
        })
    }

    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    pub fn contains(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }
}

#[derive(Debug, Clone, Default)]
pub struct EvaluateOptions {
    /// Per-node trace, used by the simulator, the decision log and AI explain.
    pub trace: bool,
    /// Replays a logged decision: its trace, from which connector nodes answer
    /// with their output at the time instead of their mock.
    pub recorded_trace: Option<Arc<Value>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Evaluation {
    pub result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
    pub performance: String,
}

#[async_trait]
pub trait DecisionRuntime: Send + Sync {
    async fn evaluate(
        &self,
        bundle: &Bundle,
        key: &str,
        context: Value,
        options: EvaluateOptions,
    ) -> Result<Evaluation, RuntimeError>;
}

/// [`DecisionRuntime`] backed by zen-engine.
///
/// Evaluation futures are not `Send` (function nodes run in an embedded
/// QuickJS), so work runs on a pool of single-threaded workers, which is the
/// same approach the upstream editor and agent use.
#[derive(Clone)]
pub struct ZenRuntime {
    pool: LocalPoolHandle,
    connectors: Arc<ConnectorAdapter>,
}

impl ZenRuntime {
    pub fn new(workers: usize) -> Self {
        Self {
            pool: LocalPoolHandle::new(workers.max(1)),
            connectors: Arc::new(ConnectorAdapter::mock()),
        }
    }
}

impl Default for ZenRuntime {
    fn default() -> Self {
        Self::new(available_parallelism().map(Into::into).unwrap_or(1))
    }
}

#[async_trait]
impl DecisionRuntime for ZenRuntime {
    async fn evaluate(
        &self,
        bundle: &Bundle,
        key: &str,
        context: Value,
        options: EvaluateOptions,
    ) -> Result<Evaluation, RuntimeError> {
        if !bundle.contains(key) {
            return Err(RuntimeError::NotFound(key.to_owned()));
        }

        let loader = bundle.loader.clone();
        let connectors = match &options.recorded_trace {
            Some(trace) => Arc::new(ConnectorAdapter::replay(recorded_outputs(trace))),
            None => self.connectors.clone(),
        };
        let key = key.to_owned();
        let trace = if options.trace {
            EvaluationTraceKind::Default
        } else {
            EvaluationTraceKind::None
        };

        let outcome = self
            .pool
            .spawn_pinned(move || async move {
                let engine = DecisionEngine::default()
                    .with_loader(loader)
                    .with_adapter(connectors);
                engine
                    .evaluate_serialized(
                        key,
                        context.into(),
                        EvaluationSerializedOptions {
                            trace,
                            max_depth: MAX_DEPTH,
                        },
                    )
                    .await
            })
            .await
            .map_err(|err| RuntimeError::Internal(err.to_string()))?;

        match outcome {
            Ok(mut doc) => Ok(Evaluation {
                result: doc
                    .get_mut("result")
                    .map(Value::take)
                    .unwrap_or(Value::Null),
                trace: doc.get_mut("trace").map(Value::take),
                performance: doc
                    .get("performance")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }),
            Err(details) => Err(RuntimeError::Evaluation { details }),
        }
    }
}
