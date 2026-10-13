//! Extractors that fail with the API's JSON error body (B6). axum's own
//! `Query`, `Path` and `Json` answer bad input with plain text; these wrap
//! them and turn the rejection into an [`ApiError`].

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::api::error::ApiError;

/// Like [`Query`], but a bad query string is a JSON 400.
#[derive(Debug)]
pub struct ApiQuery<T>(pub T);

/// Like [`Path`], but a bad path parameter is a JSON 400.
#[derive(Debug)]
pub struct ApiPath<T>(pub T);

/// Like [`Json`], but a bad body is a JSON 400, 415 or 422.
#[derive(Debug)]
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state).await?;
        Ok(Self(value))
    }
}

impl<T, S> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state).await?;
        Ok(Self(value))
    }
}

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        Ok(Self(value))
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::BadRequest(rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        ApiError::BadRequest(rejection.body_text())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        // 400 for broken JSON, 415 without the JSON content type, 422 for wrong fields
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

/// Fallback for paths that no route matches.
pub async fn not_found() -> ApiError {
    ApiError::NotFound("no such endpoint, see /docs/ for the list".to_string())
}

/// Fallback for a known path called with a method it does not support.
pub async fn method_not_allowed() -> ApiError {
    ApiError::MethodNotAllowed
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::api::test_support::{app, get_json, send};

    #[tokio::test]
    async fn unknown_route_is_a_json_404() {
        let (status, body) = get_json("/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("no such endpoint"));
    }

    #[tokio::test]
    async fn wrong_method_is_a_json_405() {
        let (status, body) = get_json("/events").await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body["error"].is_string());
    }

    #[tokio::test]
    async fn bad_query_and_path_are_json_400() {
        let (status, body) = get_json("/recommendations?user=abc").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("user"));

        let (status, body) = get_json("/users/abc").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_string());
    }

    #[tokio::test]
    async fn bad_json_bodies_keep_their_status_but_get_a_json_error() {
        let app = app().await;
        let (status, body) = send(&app, "POST", "/events", Some("{not json")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_string());

        let (status, body) = send(&app, "POST", "/events", Some(r#"{"user":"x"}"#)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"].is_string());

        let request = Request::post("/events")
            .header("content-type", "text/plain")
            .body(Body::from("{}"))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("\"error\""));
    }
}
