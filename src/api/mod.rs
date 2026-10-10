pub mod dto;
pub mod error;
pub mod handlers;
pub mod state;

use axum::Router;
use axum::routing::{get, post};
use tower_http::trace::TraceLayer;

use crate::api::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/recommendations", get(handlers::recommendations))
        .route("/events", post(handlers::add_event))
        .route("/users/{user_index}", get(handlers::user))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::db::store::Store;
    use crate::domain::test_utils::{event_of, session_of, track, user};
    use crate::engine::EngineConfig;

    async fn app() -> Router {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(1), user(2)]).await.unwrap();
        store
            .add_tracks((10..14).map(track).collect())
            .await
            .unwrap();
        store
            .add_events(vec![
                event_of(1, 10, 0),
                event_of(1, 11, 1),
                event_of(2, 10, 0),
                event_of(2, 11, 1),
                event_of(2, 12, 2),
            ])
            .await
            .unwrap();
        store
            .add_sessions(vec![session_of(1, &[10, 11]), session_of(2, &[10, 11, 12])])
            .await
            .unwrap();

        let ranker = state::train(&store, EngineConfig::lenient()).await.unwrap();
        router(AppState::new(store, ranker))
    }

    async fn send(app: &Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |text| Body::from(text.to_owned())))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    async fn get_json(uri: &str) -> (StatusCode, Value) {
        send(&app().await, "GET", uri, None).await
    }

    #[tokio::test]
    async fn health_reports_counts() {
        let (status, body) = get_json("/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["users"], 2);
        assert_eq!(body["events"], 5);
    }

    #[tokio::test]
    async fn recommendations_come_with_reasons() {
        let (status, body) = get_json("/recommendations?user=1&top=3").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"], 1);
        assert_eq!(body["history_len"], 2);
        assert_eq!(body["items"].as_array().unwrap().len(), 1);
        assert_eq!(body["items"][0]["track_index"], 12);
        assert!(!body["items"][0]["reasons"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unknown_user_is_404() {
        let (status, body) = get_json("/recommendations?user=99").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("99"));
    }

    #[tokio::test]
    async fn top_must_be_in_range() {
        let (status, _) = get_json("/recommendations?user=1&top=0").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn user_is_required() {
        let (status, _) = get_json("/recommendations").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn unknown_route_is_404() {
        let (status, _) = get_json("/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

        #[tokio::test]
    async fn posted_event_shows_up_in_the_history() {
        let app = app().await;
        let (status, body) = send(&app, "POST", "/events", Some(r#"{"user":1,"track":12}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["track"], 12);

        let (_, user) = send(&app, "GET", "/users/1", None).await;
        assert_eq!(user["history_len"], 3);
        assert_eq!(user["recent"][0]["track_index"], 12);
    }

    #[tokio::test]
    async fn event_of_an_unknown_track_is_404() {
        let app = app().await;
        let (status, body) = send(&app, "POST", "/events", Some(r#"{"user":1,"track":999}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("999"));
    }

    #[tokio::test]
    async fn user_profile_has_recent_tracks() {
        let (status, body) = get_json("/users/2").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user_index"], 2);
        assert_eq!(body["history_len"], 3);
        assert_eq!(body["recent"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn unknown_user_profile_is_404() {
        let (status, _) = get_json("/users/99").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn user_id_must_be_a_number() {
        let (status, _) = get_json("/users/abc").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}