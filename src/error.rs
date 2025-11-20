use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed")]
    AuthError,
    #[error("Database error")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("User already exists")]
    UserExists,
    #[error("Resource not found")]
    NotFound,
    #[error("Internal server error")]
    InternalError(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::AuthError => (
                StatusCode::UNAUTHORIZED,
                "Authentication failed".to_string(),
            ),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
            AppError::DatabaseError(err) => match err {
                sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("23505") => {
                    if db_err.constraint() == Some("users_email_key") {
                        (StatusCode::CONFLICT, "User already exists".to_string())
                    } else {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal server error".to_string(),
                        )
                    }
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                ),
            },
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::UserExists => (StatusCode::CONFLICT, "User already exists".to_string()),
            AppError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        tracing::error!("Error occurred: {:?}", self);

        let body = Json(json!({
            "message": error_message
        }));

        (status, body).into_response()
    }
}
