use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        AppError { status: StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::NOT_FOUND, message: msg.into() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::internal(e)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
