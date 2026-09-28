/// Liveness: answers as long as the process serves requests. Does not touch the database.
#[utoipa::path(
    get,
    path = "/health",
    tag = "operations",
    responses((status = 200, description = "The service is running", body = String, example = "ok"))
)]
pub async fn health() -> &'static str {
    "ok"
}
