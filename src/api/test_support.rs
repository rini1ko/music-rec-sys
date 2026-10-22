//! A small trained app for the API tests of every module.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use crate::api::router;
use crate::api::state::{self, AppState};
use crate::config::ServerSettings;
use crate::db::store::Store;
use crate::domain::test_utils::{event_of, track, user};
use crate::engine::EngineConfig;

/// Users 1 and 2, tracks 10-13 (all of artist 0, without names). User 1
/// played 10 and 11, user 2 played 10, 11 and 12.
pub(crate) async fn state() -> AppState {
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

    let trained = state::train(&store, EngineConfig::lenient()).await.unwrap();
    // tests poll quickly, so the rate limit is off unless a test sets its own
    AppState::new(store, trained, EngineConfig::lenient()).with_settings(ServerSettings {
        rate_per_second: 0.0,
        ..ServerSettings::default()
    })
}

pub(crate) async fn app() -> Router {
    router(state().await)
}

pub(crate) async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, Value) {
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

pub(crate) async fn get_json(uri: &str) -> (StatusCode, Value) {
    send(&app().await, "GET", uri, None).await
}
