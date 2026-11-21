//! A request id on every request: reused from `x-request-id` when the caller
//! sends a safe one, generated otherwise. It is returned in the response header,
//! attached to the tracing span (and to every exported span, see `telemetry`), and available
//! to error responses.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;

pub const HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Longer or unusual incoming ids are replaced so they cannot pollute logs.
const MAX_INCOMING_LEN: usize = 128;

tokio::task_local! {
    static CURRENT: String;
}

/// The id of the request being handled, if called within one.
pub fn current() -> Option<String> {
    CURRENT.try_with(Clone::clone).ok()
}

pub async fn middleware(req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(&HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| is_safe(v))
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let span = tracing::info_span!(
        "request",
        request_id = %id,
        method = %req.method(),
        // The route template, not the path: no ids or tokens in logs or exported spans.
        route = %crate::telemetry::route(&req),
    );
    let mut response = CURRENT
        .scope(id.clone(), next.run(req))
        .instrument(span)
        .await;

    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(HEADER, value);
    }
    response
}

fn is_safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_INCOMING_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::is_safe;

    #[test]
    fn accepts_common_id_formats() {
        assert!(is_safe("3f1c9a2e-7b1d-4c55-9a0e-1f2b3c4d5e6f"));
        assert!(is_safe("req_01HX.abc"));
    }

    #[test]
    fn rejects_ids_that_could_pollute_logs() {
        assert!(!is_safe(""));
        assert!(!is_safe("a b"));
        assert!(!is_safe("line\nbreak"));
        assert!(!is_safe(&"x".repeat(129)));
    }
}
