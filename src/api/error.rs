use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use crate::api::dto::ErrorBody;
use tokio::task::JoinError;

use crate::db::error::DbError;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Unavailable,
    Internal,
}

impl From<DbError> for ApiError {
    fn from(error: DbError) -> Self {
        match error {
            DbError::UserNotFound(id) => ApiError::NotFound(format!("user {id} not found")),
            DbError::TrackNotFound(id) => ApiError::NotFound(format!("track {id} not found")),
            DbError::Conflict(message) => ApiError::Conflict(message),
            DbError::Connection(message) => {
                tracing::error!(%message, "database is unreachable");
                ApiError::Unavailable
            }
            DbError::Storage(message) => {
                tracing::error!(%message, "storage error");
                ApiError::Internal
            }
        }
    }
}

impl From<JoinError> for ApiError {
    fn from(error: JoinError) -> Self {
        tracing::error!(%error, "background task failed");
        ApiError::Internal
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "database is unavailable".to_string(),
            ),
            ApiError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}