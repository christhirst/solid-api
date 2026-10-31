use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::AlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::InvalidArgument(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::Database(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Config(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = axum::Json(json!({
            "success": false,
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

impl From<AppError> for tonic::Status {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(msg) => tonic::Status::not_found(msg),
            AppError::AlreadyExists(msg) => tonic::Status::already_exists(msg),
            AppError::InvalidArgument(msg) => tonic::Status::invalid_argument(msg),
            AppError::Unauthorized(msg) => tonic::Status::permission_denied(msg),
            AppError::Database(err) => tonic::Status::internal(err.to_string()),
            AppError::Config(err) => tonic::Status::internal(err.to_string()),
            AppError::Internal(msg) => tonic::Status::internal(msg),
        }
    }
}
