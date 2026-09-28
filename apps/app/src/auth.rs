//! Session cookie, CSRF check and the guard in front of every protected route.

use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use donka_identity::User;

pub const SESSION_COOKIE: &str = "donka_session";

/// State-changing requests must carry this header. Browsers cannot add a custom
/// header to a cross-site request without a CORS preflight, which Studio never
/// allows, so its presence proves the request came from Studio's own pages.
pub const CSRF_HEADER: &str = "x-donka-csrf";

/// How the session cookie is written.
#[derive(Debug, Clone)]
pub struct CookieSettings {
    pub secure: bool,
    pub max_age_seconds: i64,
}

impl CookieSettings {
    pub fn session(&self, token: &str) -> HeaderValue {
        self.build(token, self.max_age_seconds)
    }

    pub fn clear(&self) -> HeaderValue {
        self.build("", 0)
    }

    fn build(&self, value: &str, max_age: i64) -> HeaderValue {
        let secure = if self.secure { "; Secure" } else { "" };
        let cookie = format!(
            "{SESSION_COOKIE}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
        );
        // Tokens are URL-safe base64, so the value is always a valid header.
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
    }
}

/// The session token sent by the browser, if any.
pub fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
}

/// Rejects state-changing requests that lack the CSRF header.
pub async fn require_csrf_header(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    let has_header = req
        .headers()
        .get(CSRF_HEADER)
        .is_some_and(|value| !value.is_empty());
    if safe || has_header {
        next.run(req).await
    } else {
        ApiError::CsrfRequired.into_response()
    }
}

/// Signed-in user, available to protected handlers as `Extension<CurrentUser>`.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user: User,
    pub(crate) token: String,
}

/// Deny by default: every route behind this layer needs a valid session.
pub async fn require_session(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = session_token(req.headers())
        .ok_or(ApiError::Unauthenticated)?
        .to_owned();
    let user = state.identity.authenticate(&token).await?;
    req.extensions_mut().insert(CurrentUser { user, token });
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_session_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; donka_session=abc_123; lang=fr"),
        );
        assert_eq!(session_token(&headers), Some("abc_123"));
    }

    #[test]
    fn empty_or_missing_cookie_is_no_session() {
        let mut headers = HeaderMap::new();
        assert_eq!(session_token(&headers), None);
        headers.insert(header::COOKIE, HeaderValue::from_static("donka_session="));
        assert_eq!(session_token(&headers), None);
    }

    #[test]
    fn cookie_attributes() {
        let settings = CookieSettings {
            secure: true,
            max_age_seconds: 60,
        };
        assert_eq!(
            settings.session("tok").to_str().unwrap(),
            "donka_session=tok; Path=/; HttpOnly; SameSite=Lax; Max-Age=60; Secure"
        );
        assert!(settings.clear().to_str().unwrap().contains("Max-Age=0"));
    }
}
