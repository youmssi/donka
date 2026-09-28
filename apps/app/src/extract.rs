use crate::error::ApiError;
use axum::extract::FromRequest;

/// `axum::Json` whose rejections use the API error shape (`400 INVALID_REQUEST`).
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);
