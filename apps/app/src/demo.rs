//! The demo option of an installation (DNK-64, `donka-admin init --demo`).
//!
//! At Studio's first start, on behalf of the first administrator, [`set_up`] imports the
//! retail-credit pack, releases it and deploys the release to staging, where the installed
//! Runtime serves it. Its bureau connector calls the bundled fake bureau, [`bureau`], which
//! `donka-admin demo-bureau` runs. Nothing else differs from a real installation.

use donka_identity::{Identity, User};
use donka_pack::{Catalogue, Packs};
use donka_project::Projects;
use donka_release::{Bump, Environment, Releases};

/// The pack the demo starts from, and the project key it gets.
pub const PACK: &str = "retail-credit";

/// Imports, releases and deploys the demo project, as the administrator `admin_email`.
/// Returns the project's key.
pub async fn set_up(
    identity: &Identity,
    projects: &Projects,
    packs: &Packs,
    releases: &Releases,
    catalogue: &Catalogue,
    admin_email: &str,
) -> Result<String, String> {
    let admin: User = identity
        .find_by_email(admin_email)
        .await
        .map_err(|err| err.to_string())?
        .filter(|user| user.is_admin)
        .ok_or_else(|| format!("{admin_email} is not an administrator"))?;
    let pack = catalogue
        .get(PACK)
        .map_err(|err| format!("the {PACK} pack is not in the catalogue: {err}"))?;
    let name = pack.manifest.name.get(admin.locale.as_str()).to_owned();
    let imported = packs
        .import(&admin, pack, PACK, &name)
        .await
        .map_err(|err| err.to_string())?;
    let access = projects
        .access(admin.id, imported.project.id)
        .await
        .map_err(|err| err.to_string())?;
    let release = releases
        .create(&access, Bump::Minor, "Demo: the pack as it ships")
        .await
        .map_err(|err| err.to_string())?;
    releases
        .deploy(&access, Environment::Staging, release.summary.id)
        .await
        .map_err(|err| err.to_string())?;
    Ok(PACK.to_owned())
}

/// The fake credit bureau the demo's connector calls. Answers are synthetic and depend only on
/// the last digit of the national identifier: most applicants are clean, `7` and `8` have
/// arrears, `9` serious ones. A request without the key the Runtime holds is refused.
pub mod bureau {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde_json::{json, Value};
    use std::sync::Arc;

    pub fn router(key: String) -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/report", post(report))
            .with_state(Arc::new(key))
    }

    async fn report(
        State(key): State<Arc<String>>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
        let sent = headers.get("x-api-key").and_then(|v| v.to_str().ok());
        if sent != Some(key.as_str()) {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "unknown key" })),
            ));
        }
        let id = body.get("nationalId").and_then(Value::as_str).unwrap_or("");
        Ok(Json(answer(id)))
    }

    /// The report for a national identifier.
    pub fn answer(national_id: &str) -> Value {
        let (active_loans, worst_days_past_due) = match national_id.chars().last() {
            Some('9') => (3, 120),
            Some('7' | '8') => (2, 45),
            _ => (1, 0),
        };
        json!({
            "checked": true,
            "activeLoans": active_loans,
            "worstDaysPastDue": worst_days_past_due,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::bureau;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt as _;

    #[test]
    fn answers_depend_on_the_last_digit() {
        assert_eq!(bureau::answer("CM-100")["worstDaysPastDue"], 0);
        assert_eq!(bureau::answer("CM-107")["worstDaysPastDue"], 45);
        assert_eq!(bureau::answer("CM-109")["worstDaysPastDue"], 120);
        assert_eq!(bureau::answer("")["checked"], true);
    }

    #[tokio::test]
    async fn the_bureau_refuses_a_request_without_its_key() {
        let call = |key: Option<&str>| {
            let mut req = Request::post("/report").header("content-type", "application/json");
            if let Some(key) = key {
                req = req.header("x-api-key", key);
            }
            bureau::router("secret".into()).oneshot(
                req.body(Body::from(json!({ "nationalId": "CM-109" }).to_string()))
                    .unwrap(),
            )
        };
        assert_eq!(call(None).await.unwrap().status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            call(Some("other")).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        let ok = call(Some("secret")).await.unwrap();
        assert_eq!(ok.status(), StatusCode::OK);
        let body = axum::body::to_bytes(ok.into_body(), 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, bureau::answer("CM-109"));
    }
}
