//! A small library of listeners, tracks and plays shared by the integration tests.
//!
//! Every test file compiles this module on its own, so a helper one file does
//! not call is not dead code.
#![allow(dead_code)]

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde_json::Value;
use tower::ServiceExt;

use music_recommendation_system::api::router;
use music_recommendation_system::api::state::{self, AppState};
use music_recommendation_system::config::ServerSettings;
use music_recommendation_system::db::store::Store;
use music_recommendation_system::domain::{ListenEvent, Track, User};
use music_recommendation_system::engine::EngineConfig;
use music_recommendation_system::etl::sessionizer::sessionize_all;

pub const KEY: &str = "integration-test-key";

/// Listeners 1-5 played the album 10, 11, 12, 13 in order, listener 6 only
/// played 10, listener 7 played two tracks of the other artist.
pub const ALBUM: [u32; 4] = [10, 11, 12, 13];
pub const LISTENERS: u32 = 7;
pub const PLAYS: usize = 5 * ALBUM.len() + 1 + 2;

pub fn start() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2009, 5, 4)
        .and_then(|day| day.and_hms_opt(12, 0, 0))
        .expect("a valid date")
}

pub fn tracks() -> Vec<Track> {
    let album = ALBUM.iter().map(|&index| Track {
        track_index: index,
        artist_index: 1,
        track_name: Some(format!("Song {index}")),
        artist_name: Some("Okean Elzy".to_string()),
    });
    let other = [20, 21].into_iter().map(|index| Track {
        track_index: index,
        artist_index: 2,
        track_name: Some(format!("Tune {index}")),
        artist_name: Some("DakhaBrakha".to_string()),
    });
    album.chain(other).collect()
}

pub fn users() -> Vec<User> {
    (1..=LISTENERS)
        .map(|user_index| User {
            user_index,
            gender: None,
            age: None,
            country: Some(
                if user_index % 2 == 0 {
                    "Germany"
                } else {
                    "Ukraine"
                }
                .to_string(),
            ),
        })
        .collect()
}

pub fn events() -> Vec<ListenEvent> {
    let play = |user_index, track_index, minutes| ListenEvent {
        user_index,
        track_index,
        timestamp: start() + Duration::minutes(minutes),
    };
    let mut events = Vec::new();
    for user in 1..=5 {
        let offset = i64::from(user) * 60;
        for (step, &track) in ALBUM.iter().enumerate() {
            events.push(play(user, track, offset + step as i64 * 4));
        }
    }
    events.push(play(6, 10, 600));
    events.push(play(7, 20, 700));
    events.push(play(7, 21, 704));
    events
}

pub async fn store() -> Store {
    let store = Store::open("sqlite::memory:")
        .await
        .expect("in-memory SQLite");
    store.add_users(users()).await.expect("users");
    store.add_tracks(tracks()).await.expect("tracks");
    store.add_events(events()).await.expect("events");
    let sessions = sessionize_all(events(), Duration::minutes(30));
    store.add_sessions(sessions).await.expect("sessions");
    store
}

/// A trained app without a rate limit, so tests may poll it quickly.
pub async fn app_with(api_key: Option<&str>) -> Router {
    let store = store().await;
    let trained = state::train(&store, EngineConfig::lenient())
        .await
        .expect("training on the fixture");
    let state =
        AppState::new(store, trained, EngineConfig::lenient()).with_settings(ServerSettings {
            api_key: api_key.map(str::to_string),
            rate_per_second: 0.0,
            ..ServerSettings::default()
        });
    router(state)
}

pub async fn app() -> Router {
    app_with(None).await
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub text: String,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
}

pub async fn call(app: &Router, request: Request<Body>) -> Reply {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a readable body");
    Reply {
        status,
        headers,
        text: String::from_utf8_lossy(&body).into_owned(),
    }
}

pub async fn get(app: &Router, uri: &str) -> Reply {
    let request = Request::get(uri)
        .body(Body::empty())
        .expect("a valid request");
    call(app, request).await
}

/// A JSON POST, signed with `key` when it is given.
pub async fn post(app: &Router, uri: &str, body: &str, key: Option<&str>) -> Reply {
    let mut request = Request::post(uri).header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("x-api-key", key);
    }
    call(
        app,
        request
            .body(Body::from(body.to_string()))
            .expect("a valid request"),
    )
    .await
}

/// Track indexes of a recommendation list, best first.
pub fn track_indexes(list: &Value) -> Vec<u64> {
    list.as_array()
        .expect("a list")
        .iter()
        .map(|item| item["track_index"].as_u64().expect("a track index"))
        .collect()
}
