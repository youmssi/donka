#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::StatusCode;
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;
use tokio::task::JoinSet;

const GRACE: &str = "grace@bank.example";
const ALAN: &str = "alan@bank.example";

fn url(path: &str) -> String {
    format!("{BASE}{path}")
}

async fn call(
    app: &TestApp,
    method: &str,
    path: &str,
    body: Option<Value>,
    session: &str,
) -> Reply {
    send(
        &app.router,
        request(method, &url(path), body.as_ref(), Some(session)),
    )
    .await
}

/// Creates a project as `session` (an administrator) and returns its id.
async fn project(app: &TestApp, session: &str, key: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": key, "name": format!("Project {key}") })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["id"].as_str().unwrap().to_owned()
}

async fn add(app: &TestApp, owner: &str, project: &str, email: &str, role: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": email, "role": role })),
        owner,
    )
    .await
}

async fn user_id(app: &TestApp, email: &str) -> String {
    app.identity
        .find_by_email(email)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_string()
}

// --- creating and listing ------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_administrator_creates_a_project_and_owns_it(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;

    let reply = call(
        &app,
        "POST",
        "/projects",
        Some(json!({ "key": "retail-scoring", "name": "  Retail scoring ", "description": "Personal loans" })),
        &admin,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    assert_eq!(reply.body["key"], "retail-scoring");
    assert_eq!(reply.body["name"], "Retail scoring");
    assert_eq!(reply.body["role"], "owner");
    assert!(reply.body["archivedAt"].is_null());

    let list = call(&app, "GET", "/projects", None, &admin).await;
    assert_eq!(list.body["total"], 1);
    assert_eq!(list.body["items"][0]["key"], "retail-scoring");
    assert_eq!(list.body["items"][0]["role"], "owner");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_administrators_create_projects(db: PgPool) {
    let app = with_database(db);
    signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let reply = call(
        &app,
        "POST",
        "/projects",
        Some(json!({ "key": "mine", "name": "Mine" })),
        &grace,
    )
    .await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn keys_are_url_safe_and_unique(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    for bad in ["Retail", "retail_scoring", "r", "retail-", "9lives"] {
        let reply = call(
            &app,
            "POST",
            "/projects",
            Some(json!({ "key": bad, "name": "x" })),
            &admin,
        )
        .await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        assert!(reply.body["fields"]["key"].is_string(), "{bad}");
    }
    project(&app, &admin, "retail").await;
    let again = call(
        &app,
        "POST",
        "/projects",
        Some(json!({ "key": "retail", "name": "Other" })),
        &admin,
    )
    .await;
    assert_error(&again, StatusCode::CONFLICT, "PROJECT_KEY_TAKEN");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_list_shows_only_my_projects_a_page_at_a_time(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    for key in ["alpha", "bravo", "charlie"] {
        project(&app, &admin, key).await;
    }
    let grace = signed_in_user(&app, GRACE, false).await;
    let bravo = project(&app, &admin, "delta").await;
    add(&app, &admin, &bravo, GRACE, "viewer").await;

    let mine = call(&app, "GET", "/projects", None, &grace).await;
    assert_eq!(mine.body["total"], 1);
    assert_eq!(mine.body["items"][0]["key"], "delta");
    assert_eq!(mine.body["items"][0]["role"], "viewer");

    let page = call(&app, "GET", "/projects?limit=2&offset=1", None, &admin).await;
    assert_eq!(page.body["total"], 4);
    let keys: Vec<&str> = page.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["bravo", "charlie"]);
}

// --- isolation between projects ------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_member_of_one_project_cannot_read_or_change_another(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let a = project(&app, &admin, "project-a").await;
    let b = project(&app, &admin, "project-b").await;
    add(&app, &admin, &a, GRACE, "owner").await;
    let admin_id = user_id(&app, ADMIN_EMAIL).await;

    let attempts: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", format!("/projects/{b}"), None),
        (
            "PATCH",
            format!("/projects/{b}"),
            Some(json!({ "name": "Mine now" })),
        ),
        ("POST", format!("/projects/{b}/archive"), None),
        ("POST", format!("/projects/{b}/restore"), None),
        ("GET", format!("/projects/{b}/members"), None),
        (
            "POST",
            format!("/projects/{b}/members"),
            Some(json!({ "email": GRACE, "role": "owner" })),
        ),
        (
            "PATCH",
            format!("/projects/{b}/members/{admin_id}"),
            Some(json!({ "role": "viewer" })),
        ),
        ("DELETE", format!("/projects/{b}/members/{admin_id}"), None),
    ];
    for (method, path, body) in attempts {
        let reply = call(&app, method, &path, body, &grace).await;
        assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");
    }

    // Not existing looks the same as not being a member, malformed ids too.
    let missing = uuid::Uuid::new_v4();
    let reply = call(&app, "GET", &format!("/projects/{missing}"), None, &grace).await;
    assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");
    let reply = call(&app, "GET", "/projects/not-a-uuid", None, &grace).await;
    assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");

    // By key: a member finds their project, another project's key looks missing.
    let by_key = call(&app, "GET", "/projects/by-key/project-a", None, &grace).await;
    assert_eq!(by_key.status, StatusCode::OK, "{}", by_key.body);
    assert_eq!(by_key.body["id"], a.as_str());
    assert_eq!(by_key.body["role"], "owner");
    for key in ["project-b", "no-such-project"] {
        let reply = call(
            &app,
            "GET",
            &format!("/projects/by-key/{key}"),
            None,
            &grace,
        )
        .await;
        assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");
    }

    // B is untouched.
    let b_now = call(&app, "GET", &format!("/projects/{b}"), None, &admin).await;
    assert_eq!(b_now.body["name"], "Project project-b");
    let members = call(&app, "GET", &format!("/projects/{b}/members"), None, &admin).await;
    assert_eq!(members.body["items"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn projects_need_a_session(db: PgPool) {
    let app = with_database(db);
    let reply = send(&app.router, request("GET", &url("/projects"), None, None)).await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

// --- roles -----------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn viewers_and_editors_cannot_change_the_project_or_its_members(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, GRACE, false).await;
    let editor = signed_in_user(&app, ALAN, false).await;
    let p = project(&app, &admin, "credit").await;
    add(&app, &admin, &p, GRACE, "viewer").await;
    add(&app, &admin, &p, ALAN, "editor").await;
    let admin_id = user_id(&app, ADMIN_EMAIL).await;

    for session in [&viewer, &editor] {
        let changes: Vec<(&str, String, Option<Value>)> = vec![
            (
                "PATCH",
                format!("/projects/{p}"),
                Some(json!({ "name": "Renamed" })),
            ),
            ("POST", format!("/projects/{p}/archive"), None),
            ("POST", format!("/projects/{p}/restore"), None),
            (
                "POST",
                format!("/projects/{p}/members"),
                Some(json!({ "email": ADMIN_EMAIL, "role": "viewer" })),
            ),
            (
                "PATCH",
                format!("/projects/{p}/members/{admin_id}"),
                Some(json!({ "role": "viewer" })),
            ),
            ("DELETE", format!("/projects/{p}/members/{admin_id}"), None),
        ];
        for (method, path, body) in changes {
            let reply = call(&app, method, &path, body, session).await;
            assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
        }
        // Reading is fine.
        let reply = call(
            &app,
            "GET",
            &format!("/projects/{p}/members"),
            None,
            session,
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
    }
    let unchanged = call(&app, "GET", &format!("/projects/{p}"), None, &admin).await;
    assert_eq!(unchanged.body["name"], "Project credit");
    assert!(unchanged.body["archivedAt"].is_null());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn owners_manage_members(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;

    let added = add(&app, &admin, &p, " Grace@Bank.example ", "editor").await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    assert_eq!(added.body["email"], GRACE);
    assert_eq!(added.body["role"], "editor");
    let grace_id = added.body["userId"].as_str().unwrap().to_owned();

    let twice = add(&app, &admin, &p, GRACE, "viewer").await;
    assert_error(&twice, StatusCode::CONFLICT, "ALREADY_MEMBER");
    let nobody = add(&app, &admin, &p, "nobody@bank.example", "viewer").await;
    assert_error(&nobody, StatusCode::UNPROCESSABLE_ENTITY, "NO_SUCH_USER");

    let changed = call(
        &app,
        "PATCH",
        &format!("/projects/{p}/members/{grace_id}"),
        Some(json!({ "role": "owner" })),
        &admin,
    )
    .await;
    assert_eq!(changed.body["role"], "owner", "{}", changed.body);

    let members = call(&app, "GET", &format!("/projects/{p}/members"), None, &admin).await;
    let emails: Vec<&str> = members.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["email"].as_str().unwrap())
        .collect();
    assert_eq!(emails, [ADMIN_EMAIL, GRACE]);

    let removed = call(
        &app,
        "DELETE",
        &format!("/projects/{p}/members/{grace_id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let gone = call(
        &app,
        "DELETE",
        &format!("/projects/{p}/members/{grace_id}"),
        None,
        &admin,
    )
    .await;
    assert_error(&gone, StatusCode::NOT_FOUND, "MEMBER_NOT_FOUND");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_project_always_keeps_an_owner(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;
    let admin_id = user_id(&app, ADMIN_EMAIL).await;

    let demote = call(
        &app,
        "PATCH",
        &format!("/projects/{p}/members/{admin_id}"),
        Some(json!({ "role": "editor" })),
        &admin,
    )
    .await;
    assert_error(&demote, StatusCode::CONFLICT, "LAST_OWNER");
    let leave = call(
        &app,
        "DELETE",
        &format!("/projects/{p}/members/{admin_id}"),
        None,
        &admin,
    )
    .await;
    assert_error(&leave, StatusCode::CONFLICT, "LAST_OWNER");

    // With a second owner, the first may leave.
    add(&app, &admin, &p, GRACE, "owner").await;
    let leave = call(
        &app,
        "DELETE",
        &format!("/projects/{p}/members/{admin_id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(leave.status, StatusCode::NO_CONTENT);
    let still = call(&app, "GET", &format!("/projects/{p}"), None, &grace).await;
    assert_eq!(still.body["role"], "owner");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn two_owners_demoting_each_other_at_once_leave_one_owner(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;
    add(&app, &admin, &p, GRACE, "owner").await;
    let admin_id = user_id(&app, ADMIN_EMAIL).await;
    let grace_id = user_id(&app, GRACE).await;

    let mut both = JoinSet::new();
    for (session, target) in [(admin.clone(), grace_id), (grace.clone(), admin_id)] {
        let router = app.router.clone();
        let path = url(&format!("/projects/{p}/members/{target}"));
        both.spawn(async move {
            let body = json!({ "role": "viewer" });
            send(
                &router,
                request("PATCH", &path, Some(&body), Some(&session)),
            )
            .await
            .status
        });
    }
    let mut statuses = Vec::new();
    while let Some(status) = both.join_next().await {
        statuses.push(status.unwrap());
    }
    statuses.sort();
    // The second change sees the first: either its author is no longer an owner
    // (403), or the target is the last owner (409). Never zero owners.
    assert_eq!(statuses[0], StatusCode::OK, "{statuses:?}");
    assert!(
        matches!(statuses[1], StatusCode::FORBIDDEN | StatusCode::CONFLICT),
        "{statuses:?}"
    );
    let (owners,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM project_members WHERE project_id = $1::uuid AND role = 'owner'",
    )
    .bind(&p)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(owners, 1);
}

// --- archive --------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_archived_project_is_read_only_until_restored(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;

    let archived = call(
        &app,
        "POST",
        &format!("/projects/{p}/archive"),
        None,
        &admin,
    )
    .await;
    assert!(archived.body["archivedAt"].is_string(), "{}", archived.body);
    let active = call(&app, "GET", "/projects", None, &admin).await;
    assert_eq!(active.body["total"], 0);
    let listed = call(&app, "GET", "/projects?archived=true", None, &admin).await;
    assert_eq!(listed.body["items"][0]["key"], "credit");

    let rename = call(
        &app,
        "PATCH",
        &format!("/projects/{p}"),
        Some(json!({ "name": "Renamed" })),
        &admin,
    )
    .await;
    assert_error(&rename, StatusCode::CONFLICT, "PROJECT_ARCHIVED");
    let member = add(&app, &admin, &p, GRACE, "viewer").await;
    assert_error(&member, StatusCode::CONFLICT, "PROJECT_ARCHIVED");

    let restored = call(
        &app,
        "POST",
        &format!("/projects/{p}/restore"),
        None,
        &admin,
    )
    .await;
    assert!(restored.body["archivedAt"].is_null());
    let rename = call(
        &app,
        "PATCH",
        &format!("/projects/{p}"),
        Some(json!({ "name": "Renamed", "description": "Now active" })),
        &admin,
    )
    .await;
    assert_eq!(rename.body["name"], "Renamed", "{}", rename.body);
    assert_eq!(rename.body["key"], "credit");
}
