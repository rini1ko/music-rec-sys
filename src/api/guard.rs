//! Protection of the API (B4): request ids, a rate limit per client, an API
//! key for writes and service endpoints, and a time limit per request. The
//! body size limit is axum's `DefaultBodyLimit`, set in the router.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tracing::Instrument;

use crate::api::error::ApiError;
use crate::api::state::AppState;

pub const REQUEST_ID: &str = "x-request-id";
pub const API_KEY: &str = "x-api-key";
/// Longest request id taken from a client; longer ones are replaced.
const MAX_REQUEST_ID: usize = 64;
/// Buckets of clients silent for this long are forgotten, so the map does not grow forever.
const IDLE_CLIENT: Duration = Duration::from_secs(600);

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// Keeps the client's `x-request-id` (if it is short and printable) or makes
/// one, puts it into every log line of the request and into the response.
pub async fn request_id(mut request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get(REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= MAX_REQUEST_ID)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("req-{:08x}", NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)));
    let header = HeaderValue::from_str(&id).unwrap_or_else(|_| HeaderValue::from_static("invalid"));
    request.headers_mut().insert(REQUEST_ID, header.clone());

    let span = tracing::info_span!("request", id = %id);
    let mut response = next.run(request).instrument(span).await;
    response.headers_mut().insert(REQUEST_ID, header);
    response
}

/// Answers 503 when a handler takes longer than the configured limit.
pub async fn time_limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(state.settings.request_timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!("request timed out");
            ApiError::Timeout.into_response()
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    refilled: Instant,
}

/// Token buckets, one per client address: `burst` requests at once, then
/// `per_second` on average.
#[derive(Debug)]
pub struct RateLimiter {
    per_second: f64,
    burst: f64,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    pub fn new(per_second: f64, burst: u32) -> Self {
        Self {
            per_second,
            burst: f64::from(burst.max(1)),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// `Ok` if the client may go on, otherwise the seconds until the next token.
    pub fn check(&self, client: &str, now: Instant) -> Result<(), u64> {
        if self.per_second <= 0.0 {
            return Ok(());
        }
        let mut buckets = self
            .buckets
            .lock()
            .expect("the rate limit lock is never poisoned");
        if buckets.len() > 10_000 {
            buckets.retain(|_, bucket| now.duration_since(bucket.refilled) < IDLE_CLIENT);
        }
        let bucket = buckets.entry(client.to_string()).or_insert(Bucket {
            tokens: self.burst,
            refilled: now,
        });
        let elapsed = now.duration_since(bucket.refilled).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.per_second).min(self.burst);
        bucket.refilled = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - bucket.tokens) / self.per_second;
            Err(wait.ceil().max(1.0) as u64)
        }
    }
}

/// Answers 429 with `Retry-After` when a client sends too much.
pub async fn rate_limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    // the address is there when the server runs with connect info; tests share one bucket
    let client = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(|| "unknown".to_string(), |info| info.0.ip().to_string());
    match state.limiter.check(&client, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(seconds) => {
            tracing::warn!(%client, "rate limit reached");
            ApiError::TooManyRequests(seconds).into_response()
        }
    }
}

/// Lets the request through only with the configured `x-api-key`.
/// Without a configured key every request passes (local development).
pub async fn require_api_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.settings.api_key.as_deref() else {
        return next.run(request).await;
    };
    let given = request.headers().get(API_KEY).map(HeaderValue::as_bytes);
    match given {
        Some(given) if same_secret(given, expected.as_bytes()) => next.run(request).await,
        _ => ApiError::Unauthorized.into_response(),
    }
}

/// Compares without stopping at the first different byte, so the time taken
/// does not tell how much of a guessed key was right. Only the length leaks.
fn same_secret(given: &[u8], expected: &[u8]) -> bool {
    given.len() == expected.len()
        && given
            .iter()
            .zip(expected)
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_compared_fully() {
        assert!(same_secret(b"abc", b"abc"));
        assert!(!same_secret(b"abd", b"abc"));
        assert!(!same_secret(b"ab", b"abc"));
        assert!(!same_secret(b"", b"abc"));
    }

    #[test]
    fn a_bucket_allows_a_burst_then_refills() {
        let limiter = RateLimiter::new(2.0, 3);
        let start = Instant::now();
        for _ in 0..3 {
            assert_eq!(limiter.check("a", start), Ok(()));
        }
        assert_eq!(limiter.check("a", start), Err(1));
        // another client has its own bucket
        assert_eq!(limiter.check("b", start), Ok(()));
        // half a second gives one token back at 2 per second
        assert_eq!(
            limiter.check("a", start + Duration::from_millis(500)),
            Ok(())
        );
    }

    #[test]
    fn zero_rate_turns_the_limit_off() {
        let limiter = RateLimiter::new(0.0, 1);
        let now = Instant::now();
        for _ in 0..100 {
            assert_eq!(limiter.check("a", now), Ok(()));
        }
    }

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use tower::ServiceExt;

    use crate::api::router;
    use crate::api::test_support::{send, state};
    use crate::config::ServerSettings;

    async fn app_with(settings: ServerSettings) -> Router {
        router(state().await.with_settings(settings))
    }

    fn keyed() -> ServerSettings {
        ServerSettings {
            api_key: Some("s3cret".to_string()),
            rate_per_second: 0.0,
            ..ServerSettings::default()
        }
    }

    #[tokio::test]
    async fn writes_need_the_key_when_one_is_set() {
        let app = app_with(keyed()).await;

        let (status, body) = send(&app, "POST", "/events", Some(r#"{"user":1,"track":12}"#)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body["error"].as_str().unwrap().contains("x-api-key"));
        let (status, _) = send(&app, "POST", "/model/retrain", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let request = Request::post("/events")
            .header("content-type", "application/json")
            .header(API_KEY, "s3cret")
            .body(Body::from(r#"{"user":1,"track":12}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::CREATED
        );

        // reads stay open, and the same path with GET is not guarded
        let (status, _) = send(&app, "GET", "/users", None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_burst_over_the_limit_gets_429_with_retry_after() {
        let app = app_with(ServerSettings {
            rate_per_second: 1.0,
            rate_burst: 2,
            ..ServerSettings::default()
        })
        .await;

        for _ in 0..2 {
            assert_eq!(send(&app, "GET", "/health", None).await.0, StatusCode::OK);
        }
        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()["retry-after"], "1");
    }

    #[tokio::test]
    async fn request_ids_are_kept_or_made() {
        let app = app_with(ServerSettings::default()).await;
        let given = Request::get("/health")
            .header(REQUEST_ID, "abc-123")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(given).await.unwrap();
        assert_eq!(response.headers()[REQUEST_ID], "abc-123");

        let made = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            made.headers()[REQUEST_ID]
                .to_str()
                .unwrap()
                .starts_with("req-")
        );
    }

    #[tokio::test]
    async fn bodies_over_the_limit_are_refused() {
        let app = app_with(ServerSettings {
            body_limit: 1024,
            ..ServerSettings::default()
        })
        .await;
        let country = "x".repeat(2000);
        let body = format!(r#"{{"country":"{country}"}}"#);
        let (status, response) = send(&app, "POST", "/users", Some(&body)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(response["error"].is_string());
    }

    #[tokio::test]
    async fn slow_handlers_are_cut_off_with_503() {
        let state = state().await.with_settings(ServerSettings {
            request_timeout: Duration::from_millis(10),
            ..ServerSettings::default()
        });
        let app = Router::new()
            .route(
                "/slow",
                get(|| async { tokio::time::sleep(Duration::from_secs(5)).await }),
            )
            .layer(from_fn_with_state(state.clone(), time_limit))
            .with_state(state);

        let (status, body) = send(&app, "GET", "/slow", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("too long"));
    }
}
