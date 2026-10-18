//! `GET /events/stream` (R5): every play stored through `POST /events` is sent
//! to the open streams as a Server-Sent Event. A slow client that falls more
//! than the channel size behind gets a `lagged` event with the number of plays
//! it missed instead of blocking anyone.

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use futures_util::stream::unfold;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{self, Receiver};
use tokio::sync::watch;

use crate::api::dto::EventDto;
use crate::api::state::AppState;

/// Plays kept for slow clients before they start to miss some.
pub const CHANNEL_SIZE: usize = 256;

pub fn channel() -> broadcast::Sender<EventDto> {
    broadcast::channel(CHANNEL_SIZE).0
}

/// What one client keeps between two events: its plays and the shutdown flag.
type Client = (Receiver<EventDto>, watch::Receiver<bool>);

/// The next SSE event of one client, `None` when the server shuts the channel
/// or starts to shut down (the sender of the channel lives in the state, so
/// the channel alone is never closed while the server runs).
async fn next_event(
    (mut receiver, mut shutdown): Client,
) -> Option<(Result<Event, Infallible>, Client)> {
    let event = tokio::select! {
        received = receiver.recv() => match received {
            Ok(play) => Event::default()
                .event("play")
                .json_data(&play)
                .unwrap_or_else(|_| Event::default().comment("a play could not be encoded")),
            Err(RecvError::Lagged(missed)) => {
                Event::default().event("lagged").data(missed.to_string())
            }
            Err(RecvError::Closed) => return None,
        },
        () = async {
            // an error means the state is gone, which ends the stream too
            let _ = shutdown.wait_for(|stopping| *stopping).await;
        } => return None,
    };
    Some((Ok(event), (receiver, shutdown)))
}

/// Plays as they are stored, as Server-Sent Events (`event: play`, JSON data
/// like the answer of `POST /events`).
#[utoipa::path(
    get,
    path = "/events/stream",
    tag = "events",
    responses((status = 200, description = "A stream of `play` events", content_type = "text/event-stream", body = EventDto))
)]
pub async fn event_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = unfold((state.plays.subscribe(), state.shutdown_watch()), next_event);
    // the keep-alive comments stop proxies from closing a quiet stream
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use futures_util::StreamExt;
    use tower::ServiceExt;

    use crate::api::router;
    use crate::api::test_support::{send, state};

    #[tokio::test]
    async fn a_stored_play_reaches_an_open_stream() {
        let app = router(state().await);
        let response = app
            .clone()
            .oneshot(Request::get("/events/stream").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let mut body = response.into_body().into_data_stream();

        let (status, _) = send(&app, "POST", "/events", Some(r#"{"user":2,"track":13}"#)).await;
        assert_eq!(status, StatusCode::CREATED);

        let chunk = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .expect("an event arrives")
            .unwrap()
            .unwrap();
        let text = String::from_utf8(chunk.to_vec()).unwrap();
        assert!(text.contains("event: play"), "{text}");
        assert!(text.contains(r#""track":13"#), "{text}");
    }

    #[tokio::test]
    async fn an_open_stream_ends_when_the_server_shuts_down() {
        let state = state().await;
        let app = router(state.clone());
        let response = app
            .oneshot(Request::get("/events/stream").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let mut body = response.into_body().into_data_stream();

        state.begin_shutdown();

        let end = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .expect("the stream ends instead of waiting for a play");
        assert!(end.is_none());
    }

    #[tokio::test]
    async fn a_slow_client_is_told_how_much_it_missed() {
        let state = state().await;
        let app = router(state.clone());
        let response = app
            .oneshot(Request::get("/events/stream").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let mut body = response.into_body().into_data_stream();

        // the client reads nothing while the channel overflows by 3
        for _ in 0..super::CHANNEL_SIZE + 3 {
            let _ = state.plays.send(crate::api::dto::EventDto {
                user: 1,
                track: 10,
                timestamp: chrono::NaiveDateTime::default(),
            });
        }
        let chunk = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = String::from_utf8(chunk.to_vec()).unwrap();
        assert!(text.contains("event: lagged"), "{text}");
        assert!(text.contains("data: 3"), "{text}");
    }
}
