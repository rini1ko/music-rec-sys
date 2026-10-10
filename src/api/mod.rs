pub mod state;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;

use crate::api::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Also proves that the database answers.
async fn health(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let counts = state.store.counts().await.map_err(|error| {
        tracing::error!(%error, "health check failed");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    Ok(Json(json!({
        "status": "ok",
        "users": counts.users,
        "tracks": counts.tracks,
        "events": counts.events,
        "sessions": counts.sessions,
    })))
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::db::store::Store;

    #[tokio::test]
    async fn health_reports_counts() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let app = router(AppState { store });

        let request = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["status"], "ok");
        assert_eq!(value["events"], 0);
    }

    #[tokio::test]
    async fn unknown_route_is_404() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let app = router(AppState { store });

        let request = Request::builder().uri("/nope").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}