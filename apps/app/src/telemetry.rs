//! Logs, and optionally traces and request metrics over OTLP (DNK-27).
//!
//! Logs always go to standard output. Traces and metrics are exported only when
//! `DONKA_OTEL_ENABLED=true`; where to and how come from the standard OpenTelemetry variables
//! (`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_SERVICE_NAME`…), so an
//! operator's collector setup works unchanged. Nothing leaves the process by default.
//!
//! Spans carry the request id, the HTTP method and the route template (`/projects/{id}`), never
//! the raw path, the query string, headers or bodies: a password-setup link carries its token in
//! the query, and request bodies hold applicants' data.

use crate::request_id;
use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::metrics::Histogram;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, Context, KeyValue};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::{Span, SpanData, SpanProcessor};
use opentelemetry_sdk::Resource;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Span attribute holding the request id; the same value as the `x-request-id` header.
pub const REQUEST_ID: &str = "donka.request_id";

const DEFAULT_FILTER: &str = "warn,donka_app=info,donka_identity=info,tower_http=info";

/// Exporters to flush on shutdown; empty when telemetry is off.
#[derive(Default)]
pub struct Telemetry {
    tracer: Option<SdkTracerProvider>,
    meter: Option<SdkMeterProvider>,
}

impl Telemetry {
    /// Sends what is still buffered, so the last requests before a stop are not lost.
    pub fn shutdown(self) {
        if let Some(tracer) = self.tracer {
            if let Err(error) = tracer.shutdown() {
                tracing::warn!(%error, "traces could not be flushed");
            }
        }
        if let Some(meter) = self.meter {
            if let Err(error) = meter.shutdown() {
                tracing::warn!(%error, "metrics could not be flushed");
            }
        }
    }
}

/// Installs logging, and OTLP export of traces and metrics when `otel_enabled`.
pub fn init(otel_enabled: bool) -> Result<Telemetry, String> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let logs = tracing_subscriber::fmt::layer();
    if !otel_enabled {
        tracing_subscriber::registry()
            .with(filter)
            .with(logs)
            .init();
        return Ok(Telemetry::default());
    }

    let resource = Resource::builder()
        .with_service_name("donka-studio")
        .build();
    let spans = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .map_err(|error| format!("OTLP traces: {error}"))?;
    let tracer = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_span_processor(RequestIdProcessor)
        .with_batch_exporter(spans)
        .build();
    let metrics = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .build()
        .map_err(|error| format!("OTLP metrics: {error}"))?;
    let meter = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_periodic_exporter(metrics)
        .build();
    global::set_meter_provider(meter.clone());

    let traces = tracing_opentelemetry::layer().with_tracer(tracer.tracer("donka-studio"));
    tracing_subscriber::registry()
        .with(filter)
        .with(logs)
        .with(traces)
        .init();
    Ok(Telemetry {
        tracer: Some(tracer),
        meter: Some(meter),
    })
}

/// Stamps the id of the request being handled on every span started while handling it, so a
/// trace can be found from the id a user quotes, whichever span they start from.
#[derive(Debug)]
pub struct RequestIdProcessor;

impl SpanProcessor for RequestIdProcessor {
    fn on_start(&self, span: &mut Span, _cx: &Context) {
        if let Some(id) = request_id::current() {
            opentelemetry::trace::Span::set_attribute(span, KeyValue::new(REQUEST_ID, id));
        }
    }

    fn on_end(&self, _span: SpanData) {}

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }
}

/// The route template the request matched, or `unmatched` (static files, unknown paths).
pub fn route<B>(req: &axum::http::Request<B>) -> String {
    req.extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |path| path.as_str().to_owned())
}

/// `http.server.request.duration`, by method, route template and status: what a dashboard needs
/// for rates, errors and latency. A no-op unless telemetry is on.
pub async fn record_request(req: Request, next: Next) -> Response {
    static DURATION: OnceLock<Histogram<f64>> = OnceLock::new();
    let method = req.method().as_str().to_owned();
    let route = route(&req);
    let started = Instant::now();
    let response = next.run(req).await;
    DURATION
        .get_or_init(|| {
            global::meter("donka-studio")
                .f64_histogram("http.server.request.duration")
                .with_unit("s")
                .with_description("Duration of HTTP requests handled by Studio")
                .build()
        })
        .record(
            started.elapsed().as_secs_f64(),
            &[
                KeyValue::new("http.request.method", method),
                KeyValue::new("http.route", route),
                KeyValue::new(
                    "http.response.status_code",
                    i64::from(response.status().as_u16()),
                ),
            ],
        );
    response
}
