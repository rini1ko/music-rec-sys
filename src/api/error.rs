use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio::task::JoinError;

use crate::api::dto::ErrorBody;
use crate::db::error::DbError;

/// Every error the API answers with. All of them become `{"error": "..."}`.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    MethodNotAllowed,
    /// The request is fine but needs a valid `x-api-key` header.
    Unauthorized,
    /// Too many requests from one client; the number is the seconds to wait.
    TooManyRequests(u64),
    /// The handler took longer than the configured limit.
    Timeout,
    /// A rejection that carries its own status, e.g. 415 or 422 for a JSON body.
    Rejected(StatusCode, String),
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

impl ApiError {
    fn status_and_message(self) -> (StatusCode, String) {
        match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "this endpoint does not support the method".to_string(),
            ),
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "a valid x-api-key header is required".to_string(),
            ),
            ApiError::TooManyRequests(seconds) => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("too many requests, try again in {seconds} s"),
            ),
            ApiError::Timeout => (
                StatusCode::SERVICE_UNAVAILABLE,
                "the request took too long".to_string(),
            ),
            ApiError::Rejected(status, message) => (status, message),
            ApiError::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "database is unavailable".to_string(),
            ),
            ApiError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let retry_after = match &self {
            ApiError::TooManyRequests(seconds) => Some(*seconds),
            _ => None,
        };
        let (status, message) = self.status_and_message();
        let mut response = (status, Json(ErrorBody { error: message })).into_response();
        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, seconds.into());
        }
        response
    }
}
