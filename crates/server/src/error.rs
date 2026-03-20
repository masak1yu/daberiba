use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal server error")]
    Internal(#[from] anyhow::Error),

    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, errcode, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "M_NOT_FOUND", self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "M_UNKNOWN_TOKEN", self.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "M_FORBIDDEN", self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "M_BAD_JSON", msg.clone()),
            AppError::Internal(_) | AppError::Database(_) => {
                tracing::error!(error = %self);
                (StatusCode::INTERNAL_SERVER_ERROR, "M_UNKNOWN", "internal server error".to_string())
            }
        };

        let body = Json(json!({
            "errcode": errcode,
            "error": message,
        }));

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, AppError>;
