#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! What Studio exports over OpenTelemetry (DNK-27): spans and request metrics carry the request
//! id and the route template, never a raw path, a query string or a token. The global meter and
//! the tracing subscriber are process-wide, so these tests have their own binary.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use donka_app::telemetry::{RequestIdProcessor, REQUEST_ID};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Value;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use std::sync::OnceLock;
use support::*;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const SECRET: &str = "s3cr3t-setup-token";

fn request(path: &str, id: &str) -> Request<Body> {
    Request::get(path)
        .header("x-request-id", id)
        .body(Body::empty())
        .unwrap()
}

fn text(value: &Value) -> String {
    value.as_str().into_owned()
}

/// One tracing subscriber for the whole binary: a thread-local one would miss spans whose
/// callsites another test thread reached first.
fn spans() -> &'static (InMemorySpanExporter, SdkTracerProvider) {
    static SPANS: OnceLock<(InMemorySpanExporter, SdkTracerProvider)> = OnceLock::new();
    SPANS.get_or_init(|| {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(RequestIdProcessor)
            .with_simple_exporter(exporter.clone())
            .build();
        tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")))
            .init();
        (exporter, provider)
    })
}

#[tokio::test]
async fn every_span_carries_the_request_id_and_no_path_or_secret() {
    let (exporter, provider) = spans();
    let app = without_database();
    // A token in the query, as a password-setup link carries it, and ids in the path.
    let health = send(
        &app.router,
        request(&format!("/api/v1/health?token={SECRET}"), "req-health"),
    )
    .await;
    assert_eq!(health.status, StatusCode::OK);
    let project = send(
        &app.router,
        request(
            "/api/v1/projects/credit-pme/decisions/7f2c546b",
            "req-project",
        ),
    )
    .await;
    assert_eq!(project.status, StatusCode::UNAUTHORIZED);
    provider.force_flush().unwrap();

    // Only this test's requests: the other test's spans land in the same exporter.
    let spans: Vec<_> = exporter
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .filter(|span| {
            span.attributes
                .iter()
                .any(|kv| matches!(text(&kv.value).as_str(), "req-health" | "req-project"))
        })
        .collect();
    assert!(spans.len() >= 4, "{} spans", spans.len());
    for span in &spans {
        let id = span
            .attributes
            .iter()
            .find(|kv| kv.key.as_str() == REQUEST_ID || kv.key.as_str() == "request_id")
            .map(|kv| text(&kv.value));
        assert!(
            matches!(id.as_deref(), Some("req-health" | "req-project")),
            "span {} has no request id: {:?}",
            span.name,
            span.attributes
        );
        for kv in &span.attributes {
            let value = text(&kv.value);
            assert!(
                !value.contains(SECRET),
                "{} leaks the token: {value}",
                span.name
            );
            assert!(
                !value.contains("credit-pme"),
                "{} leaks the path: {value}",
                span.name
            );
            assert!(!value.contains('?'), "{} leaks a query: {value}", span.name);
        }
    }
    let routes: Vec<String> = spans
        .iter()
        .flat_map(|span| span.attributes.iter())
        .filter(|kv| kv.key.as_str() == "route")
        .map(|kv| text(&kv.value))
        .collect();
    assert!(
        routes.iter().any(|route| route == "/api/v1/health"),
        "{routes:?}"
    );
    assert!(
        routes
            .iter()
            .any(|route| route.starts_with("/api/v1/projects/{")),
        "{routes:?}"
    );
}

#[tokio::test]
async fn request_metrics_are_recorded_by_route_template() {
    let exporter = InMemoryMetricExporter::default();
    spans();
    let provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter.clone()).build())
        .build();
    opentelemetry::global::set_meter_provider(provider.clone());

    let app = without_database();
    send(&app.router, request("/api/v1/health", "m-1")).await;
    send(&app.router, request("/api/v1/projects/credit-pme", "m-2")).await;
    provider.force_flush().unwrap();

    let mut seen = Vec::new();
    for resource in exporter.get_finished_metrics().unwrap() {
        for scope in resource.scope_metrics() {
            for metric in scope.metrics() {
                assert_eq!(metric.name(), "http.server.request.duration");
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("not a histogram");
                };
                for point in histogram.data_points() {
                    let attribute = |key: &str| {
                        point
                            .attributes()
                            .find(|kv| kv.key.as_str() == key)
                            .map(|kv| text(&kv.value))
                            .unwrap_or_default()
                    };
                    seen.push((
                        attribute("http.request.method"),
                        attribute("http.route"),
                        attribute("http.response.status_code"),
                    ));
                }
            }
        }
    }
    assert!(
        seen.contains(&("GET".into(), "/api/v1/health".into(), "200".into())),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(_, route, status)| route.starts_with("/api/v1/projects/{") && status == "401"),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .all(|(_, route, _)| !route.contains("credit-pme")),
        "{seen:?}"
    );
}
