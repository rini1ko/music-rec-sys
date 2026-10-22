//! The HTTP API from the outside: several requests in a row, the way the
//! frontend sends them, against a trained app on an in-memory database.

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;

use common::{ALBUM, KEY, LISTENERS, PLAYS, app, app_with, get, post, track_indexes};

/// Every route of the router, as the OpenAPI document must name it.
const ROUTES: [&str; 22] = [
    "/health",
    "/metrics",
    "/recommendations",
    "/recommendations/compare",
    "/events",
    "/events/stream",
    "/users",
    "/users/{user_index}",
    "/users/{user_index}/feedback",
    "/users/{user_index}/history",
    "/users/{user_index}/radio",
    "/model",
    "/model/retrain",
    "/model/evaluate",
    "/model/evaluation",
    "/feedback",
    "/tracks",
    "/tracks/{track_index}",
    "/tracks/{track_index}/similar",
    "/stats/overview",
    "/artists",
    "/artists/{artist_index}",
];

fn sources(item: &Value) -> Vec<&str> {
    item["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .map(|reason| reason["source"].as_str().expect("a source"))
        .collect()
}

#[tokio::test]
async fn health_counts_the_loaded_library() {
    let app = app().await;
    let reply = get(&app, "/health").await;
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["users"], LISTENERS);
    assert_eq!(body["tracks"], 6);
    assert_eq!(body["events"], PLAYS);
    // one session per listener: their plays are minutes apart
    assert_eq!(body["sessions"], LISTENERS);
}

#[tokio::test]
async fn the_next_track_of_the_album_comes_first() {
    let app = app().await;
    let reply = get(&app, "/recommendations?user=6&top=3").await;
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["history_len"], 1);

    let items = &body["items"];
    let order = track_indexes(items);
    assert_eq!(order.first(), Some(&11), "got {order:?}");
    assert!(!order.contains(&10), "a heard track came back: {order:?}");

    let scores: Vec<f64> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["score"].as_f64().unwrap())
        .collect();
    assert!(
        scores.windows(2).all(|pair| pair[0] >= pair[1]),
        "{scores:?}"
    );
    assert!(sources(&items[0]).contains(&"markov"));
    assert_eq!(items[0]["title"], "Song 11");
    assert_eq!(items[0]["artist"], "Okean Elzy");
}

#[tokio::test]
async fn a_play_changes_the_next_list_without_retraining() {
    let app = app().await;
    let before = track_indexes(&get(&app, "/recommendations?user=6&top=3").await.json()["items"]);
    assert_eq!(before.first(), Some(&11));

    let reply = post(&app, "/events", r#"{"user": 6, "track": 11}"#, None).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    // without a timestamp the play goes one minute after the newest one
    assert_eq!(reply.json()["timestamp"], "2009-05-04T23:45:00");

    // the cached list is dropped, and the history now ends with 11
    let body = get(&app, "/recommendations?user=6&top=3").await.json();
    assert_eq!(body["history_len"], 2);
    let after = track_indexes(&body["items"]);
    assert!(!after.contains(&11), "{after:?}");
    assert_eq!(after.first(), Some(&12), "{after:?}");
}

#[tokio::test]
async fn a_new_listener_gets_popular_tracks_and_then_a_history() {
    let app = app().await;
    let created = post(&app, "/users", r#"{"country": "Ukraine", "age": 30}"#, None).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let user = created.json()["user_index"].as_u64().unwrap();
    assert!(user > u64::from(LISTENERS));

    let body = get(&app, &format!("/recommendations?user={user}&top=5"))
        .await
        .json();
    assert_eq!(body["history_len"], 0);
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
    for item in items {
        let found = sources(item);
        assert!(
            found
                .iter()
                .all(|source| ["popular", "country"].contains(source)),
            "{found:?}"
        );
    }

    let played = post(
        &app,
        "/events",
        &format!(r#"{{"user": {user}, "track": 20}}"#),
        None,
    )
    .await;
    assert_eq!(played.status, StatusCode::CREATED);
    let profile = get(&app, &format!("/users/{user}")).await.json();
    assert_eq!(profile["history_len"], 1);
    assert_eq!(profile["country"], "Ukraine");
    assert_eq!(profile["recent"][0]["track_index"], 20);

    let users = get(&app, "/users?limit=50").await.json();
    assert_eq!(users["total"], u64::from(LISTENERS) + 1);
}

#[tokio::test]
async fn writes_need_the_key_and_reads_do_not() {
    let app = app_with(Some(KEY)).await;
    let play = r#"{"user": 6, "track": 12}"#;

    for key in [None, Some("wrong")] {
        let reply = post(&app, "/events", play, key).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "key {key:?}");
        assert!(reply.json()["error"].is_string());
    }
    assert_eq!(
        post(&app, "/model/retrain", "", None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(&app, "/users", "{}", None).await.status,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        post(&app, "/events", play, Some(KEY)).await.status,
        StatusCode::CREATED
    );
    assert_eq!(get(&app, "/health").await.status, StatusCode::OK);
    assert_eq!(get(&app, "/users/6").await.json()["history_len"], 2);
}

#[tokio::test]
async fn retraining_counts_the_new_plays() {
    let app = app().await;
    assert_eq!(get(&app, "/model").await.json()["events"], PLAYS);
    post(&app, "/events", r#"{"user": 7, "track": 10}"#, None).await;

    let started = post(&app, "/model/retrain", "", None).await;
    assert_eq!(started.status, StatusCode::ACCEPTED);

    let mut status = Value::Null;
    for _ in 0..200 {
        status = get(&app, "/model").await.json();
        if status["training"] == false && status["events"] == PLAYS + 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(status["training"], false);
    assert_eq!(status["events"], PLAYS + 1);
    assert!(status["last_error"].is_null());
}

#[tokio::test]
async fn likes_are_stored_and_drive_the_bandit() {
    let app = app().await;
    // the recommended track is credited to a model, so the like has a source
    get(&app, "/recommendations?user=6&top=3").await;
    let liked = post(
        &app,
        "/feedback",
        r#"{"user": 6, "track": 11, "kind": "like"}"#,
        None,
    )
    .await;
    assert_eq!(liked.status, StatusCode::CREATED);
    assert_eq!(liked.json()["kind"], "like");
    assert!(liked.json()["source"].is_string());

    let feedback = get(&app, "/users/6/feedback").await.json();
    assert_eq!(feedback["items"].as_array().unwrap().len(), 1);
    assert_eq!(feedback["items"][0]["track_index"], 11);
    assert_eq!(feedback["items"][0]["title"], "Song 11");

    let body = get(&app, "/recommendations?user=6&top=3&bandit=true")
        .await
        .json();
    let weights = &body["bandit_weights"];
    assert!(weights.is_object(), "{body}");
    for model in ["trends", "markov", "co_listening", "cf", "mf"] {
        assert!(weights[model].as_f64().is_some(), "{model} missing");
    }

    let unknown = post(
        &app,
        "/feedback",
        r#"{"user": 6, "track": 999, "kind": "skip"}"#,
        None,
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn other_weights_reorder_the_same_list() {
    let app = app().await;
    let uri = "/recommendations/compare?user=6&top=4&trends=1&markov=0&co_listening=0&cf=0&mf=0";
    let reply = get(&app, uri).await;
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["weights"]["trends"], 1.0);
    assert_eq!(body["weights"]["markov"], 0.0);

    let base = body["base"].as_array().unwrap();
    let custom = body["custom"].as_array().unwrap();
    assert!(!base.is_empty() && !custom.is_empty());
    let changed = ["up", "down", "added", "gone"]
        .iter()
        .map(|field| body[*field].as_u64().unwrap())
        .sum::<u64>();
    let moved = custom
        .iter()
        .filter(|item| item["movement"]["kind"] != "same")
        .count();
    // `gone` counts tracks that left the list, they have no row in `custom`
    assert!(changed >= moved as u64);
    // the other models are off; the artist and country rules still add their bonus
    for item in custom {
        let found = sources(item);
        assert!(found.contains(&"trends"), "{item}");
        assert!(
            !found
                .iter()
                .any(|source| ["markov", "co_listening", "cf", "mf"].contains(source)),
            "{item}"
        );
    }
}

#[tokio::test]
async fn track_artist_and_overview_pages_agree() {
    let app = app().await;
    let similar = get(&app, "/tracks/10/similar?top=3").await.json();
    assert_eq!(track_indexes(&similar).first(), Some(&11));

    let track = get(&app, "/tracks/10").await;
    assert_eq!(track.status, StatusCode::OK);
    assert_eq!(track.json()["title"], "Song 10");

    let artist = get(&app, "/artists/1").await.json();
    assert_eq!(artist["name"], "Okean Elzy");
    assert_eq!(artist["tracks"], ALBUM.len());
    assert_eq!(artist["plays"], 5 * ALBUM.len() + 1);
    assert_eq!(artist["listeners"], 6);

    let found = get(&app, "/artists?q=dakha").await.json();
    assert_eq!(found[0]["artist_index"], 2);
    let tracks = get(&app, "/tracks?q=song%2012").await.json();
    assert_eq!(track_indexes(&tracks), vec![12]);

    let overview = get(&app, "/stats/overview?days=7").await.json();
    assert_eq!(overview["plays"], PLAYS);
    assert_eq!(overview["listeners"], LISTENERS);
    assert_eq!(overview["by_hour"].as_array().unwrap().len(), 24);
    assert_eq!(overview["top_artists"][0]["name"], "Okean Elzy");
    let by_day: u64 = overview["by_day"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["plays"].as_u64().unwrap())
        .sum();
    assert_eq!(by_day, PLAYS as u64);

    let history = get(&app, "/users/1/history").await;
    assert_eq!(history.status, StatusCode::OK);
    let radio = get(&app, "/users/6/radio").await;
    assert_eq!(radio.status, StatusCode::OK);
}

#[tokio::test]
async fn every_error_is_json_with_a_message() {
    let app = app().await;
    let delete = Request::delete("/health").body(Body::empty()).unwrap();
    let cases = [
        (get(&app, "/no/such/page").await, StatusCode::NOT_FOUND),
        (
            common::call(&app, delete).await,
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (
            get(&app, "/recommendations?user=abc").await,
            StatusCode::BAD_REQUEST,
        ),
        (
            get(&app, "/recommendations?user=6&top=0").await,
            StatusCode::BAD_REQUEST,
        ),
        (
            get(&app, "/recommendations?user=999").await,
            StatusCode::NOT_FOUND,
        ),
        (
            get(&app, "/stats/overview?days=0").await,
            StatusCode::BAD_REQUEST,
        ),
        (
            post(&app, "/events", "{", None).await,
            StatusCode::BAD_REQUEST,
        ),
        (
            post(&app, "/events", r#"{"user": 6, "track": 999}"#, None).await,
            StatusCode::NOT_FOUND,
        ),
        (
            post(&app, "/users", r#"{"age": 500}"#, None).await,
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (reply, expected) in cases {
        assert_eq!(reply.status, expected, "{}", reply.text);
        let message = reply.json()["error"].as_str().map(str::to_string);
        assert!(
            message.is_some_and(|text| !text.is_empty()),
            "{}",
            reply.text
        );
    }
}

#[tokio::test]
async fn the_api_document_names_every_route() {
    let app = app().await;
    let document = get(&app, "/api-docs/openapi.json").await.json();
    let paths = document["paths"].as_object().expect("paths");
    for route in ROUTES {
        assert!(paths.contains_key(route), "{route} is not documented");
    }
    assert_eq!(
        paths.len(),
        ROUTES.len(),
        "{:?}",
        paths.keys().collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn request_ids_and_metrics_follow_each_call() {
    let app = app().await;
    let request = Request::get("/health")
        .header("x-request-id", "trace-me")
        .body(Body::empty())
        .unwrap();
    let reply = common::call(&app, request).await;
    assert_eq!(reply.headers["x-request-id"], "trace-me");
    // without one the server makes an id up
    assert!(
        get(&app, "/health")
            .await
            .headers
            .contains_key("x-request-id")
    );

    let metrics = get(&app, "/metrics").await;
    assert_eq!(metrics.status, StatusCode::OK);
    let line = metrics
        .text
        .lines()
        .find(|line| line.starts_with("http_requests_total") && line.contains("/health"))
        .expect("a counter for /health");
    assert!(line.ends_with(" 2"), "{line}");
}

#[tokio::test]
async fn the_event_stream_is_server_sent_events() {
    let app = app().await;
    let request = Request::get("/events/stream").body(Body::empty()).unwrap();
    let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
}
