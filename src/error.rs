use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Internal error: {0}")]
    Internal(String),
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
            AppError::Serialization(err) => tonic::Status::invalid_argument(err.to_string()),
            AppError::Internal(msg) => tonic::Status::internal(msg),
        }
    }
}
