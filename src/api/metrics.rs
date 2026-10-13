//! `GET /metrics` in the Prometheus text format (B2). Request counters and
//! latencies are collected by a middleware per route template
//! (`/users/{user_index}`, not `/users/7`), so the number of series stays small.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::state::AppState;

const LOCK: &str = "the metrics lock is never poisoned";

/// Upper bounds of the latency buckets, in seconds.
const BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

#[derive(Debug, Default)]
struct RouteStats {
    /// Responses by status code.
    statuses: BTreeMap<u16, u64>,
    /// Requests that took at most `BUCKETS[i]` seconds (not cumulative here).
    buckets: [u64; BUCKETS.len()],
    seconds: f64,
    count: u64,
}

/// Counters shared by every request.
#[derive(Debug, Default)]
pub struct Metrics {
    routes: Mutex<BTreeMap<(String, String), RouteStats>>,
}

impl Metrics {
    pub fn record(&self, method: &str, route: &str, status: u16, seconds: f64) {
        let mut routes = self.routes.lock().expect(LOCK);
        let stats = routes
            .entry((method.to_string(), route.to_string()))
            .or_default();
        *stats.statuses.entry(status).or_default() += 1;
        if let Some(bucket) = BUCKETS.iter().position(|&bound| seconds <= bound) {
            stats.buckets[bucket] += 1;
        }
        stats.seconds += seconds;
        stats.count += 1;
    }

    /// The request series in the Prometheus text format.
    fn render_requests(&self, out: &mut String) {
        let routes = self.routes.lock().expect(LOCK);
        out.push_str("# HELP http_requests_total Requests by route, method and status.\n");
        out.push_str("# TYPE http_requests_total counter\n");
        for ((method, route), stats) in routes.iter() {
            for (status, count) in &stats.statuses {
                let labels = labels(&[
                    ("method", method),
                    ("route", route),
                    ("status", &status.to_string()),
                ]);
                let _ = writeln!(out, "http_requests_total{labels} {count}");
            }
        }

        out.push_str("# HELP http_request_duration_seconds Time to answer, by route and method.\n");
        out.push_str("# TYPE http_request_duration_seconds histogram\n");
        for ((method, route), stats) in routes.iter() {
            let mut cumulative = 0;
            for (bound, count) in BUCKETS.iter().zip(stats.buckets) {
                cumulative += count;
                let labels = labels(&[
                    ("method", method),
                    ("route", route),
                    ("le", &bound.to_string()),
                ]);
                let _ = writeln!(
                    out,
                    "http_request_duration_seconds_bucket{labels} {cumulative}"
                );
            }
            let base = [("method", method.as_str()), ("route", route.as_str())];
            let all = labels(&[base[0], base[1], ("le", "+Inf")]);
            let _ = writeln!(
                out,
                "http_request_duration_seconds_bucket{all} {}",
                stats.count
            );
            let labels = labels(&base);
            let _ = writeln!(
                out,
                "http_request_duration_seconds_sum{labels} {}",
                stats.seconds
            );
            let _ = writeln!(
                out,
                "http_request_duration_seconds_count{labels} {}",
                stats.count
            );
        }
    }
}

/// `{a="1",b="2"}` with the values escaped as the format requires.
fn labels(pairs: &[(&str, &str)]) -> String {
    let inner: Vec<String> = pairs
        .iter()
        .map(|(name, value)| {
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            format!("{name}=\"{escaped}\"")
        })
        .collect();
    format!("{{{}}}", inner.join(","))
}

fn gauge(out: &mut String, name: &str, help: &str, value: impl std::fmt::Display) {
    let _ = writeln!(
        out,
        "# HELP {name} {help}\n# TYPE {name} gauge\n{name} {value}"
    );
}

/// Middleware: times every routed request and counts its status.
pub async fn track_requests(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_string(), |path| path.as_str().to_string());

    let response = next.run(request).await;
    state.metrics.record(
        &method,
        &route,
        response.status().as_u16(),
        started.elapsed().as_secs_f64(),
    );
    response
}

/// Request counters and latencies, the state of the models and of the cache.
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "system",
    responses((status = 200, description = "Prometheus text format", body = String, content_type = "text/plain"))
)]
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let mut out = String::with_capacity(4096);
    state.metrics.render_requests(&mut out);

    let status = state.status();
    gauge(
        &mut out,
        "model_training",
        "1 while a training runs.",
        u8::from(status.training),
    );
    gauge(
        &mut out,
        "model_training_seconds",
        "Duration of the last training.",
        status.train_seconds,
    );
    gauge(
        &mut out,
        "model_trained_events",
        "Events the models were trained on.",
        status.events,
    );
    gauge(
        &mut out,
        "model_trained_sessions",
        "Sessions the models were trained on.",
        status.sessions,
    );
    gauge(
        &mut out,
        "model_online_events",
        "Plays counted by the Markov and trends models since the training.",
        state.online_events(),
    );
    gauge(
        &mut out,
        "model_evaluation_running",
        "1 while an evaluation runs.",
        u8::from(state.evaluation().running),
    );

    let sizes = state
        .recommender
        .read()
        .expect("the recommender lock is never poisoned")
        .model_sizes();
    out.push_str("# HELP model_size What each model keeps: transitions, edges, users, tracks.\n");
    out.push_str("# TYPE model_size gauge\n");
    for (source, size) in sizes {
        let _ = writeln!(
            out,
            "model_size{} {size}",
            labels(&[("source", source.name())])
        );
    }

    let (hits, misses, entries) = state.cache.stats();
    gauge(
        &mut out,
        "recommendation_cache_entries",
        "Lists kept in the cache.",
        entries,
    );
    let _ = writeln!(
        out,
        "# HELP recommendation_cache_requests_total Cache lookups by result.\n# TYPE recommendation_cache_requests_total counter\nrecommendation_cache_requests_total{{result=\"hit\"}} {hits}\nrecommendation_cache_requests_total{{result=\"miss\"}} {misses}"
    );

    ([(CONTENT_TYPE, "text/plain; version=0.0.4")], out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::router;
    use crate::api::test_support::{send, state};
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn the_endpoint_reports_routes_models_and_cache() {
        let app = router(state().await);
        send(&app, "GET", "/users/1", None).await;
        send(&app, "GET", "/users/99", None).await;

        let response = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/plain")
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // the route template, not the concrete path, so ids do not explode the series
        assert!(text.contains(
            r#"http_requests_total{method="GET",route="/users/{user_index}",status="200"} 1"#
        ));
        assert!(text.contains(
            r#"http_requests_total{method="GET",route="/users/{user_index}",status="404"} 1"#
        ));
        assert!(text.contains(r#"model_size{source="markov"}"#));
        assert!(text.contains("model_training 0"));
        assert!(text.contains(r#"recommendation_cache_requests_total{result="hit"} 0"#));
    }

    #[test]
    fn latencies_fall_into_cumulative_buckets() {
        let metrics = Metrics::default();
        metrics.record("GET", "/health", 200, 0.003);
        metrics.record("GET", "/health", 200, 0.2);
        metrics.record("GET", "/health", 404, 20.0);

        let mut out = String::new();
        metrics.render_requests(&mut out);
        assert!(
            out.contains(r#"http_requests_total{method="GET",route="/health",status="200"} 2"#)
        );
        assert!(
            out.contains(r#"http_requests_total{method="GET",route="/health",status="404"} 1"#)
        );
        assert!(out.contains(
            r#"http_request_duration_seconds_bucket{method="GET",route="/health",le="0.005"} 1"#
        ));
        assert!(out.contains(
            r#"http_request_duration_seconds_bucket{method="GET",route="/health",le="0.25"} 2"#
        ));
        // the slow one is above the last bound and only in +Inf
        assert!(out.contains(
            r#"http_request_duration_seconds_bucket{method="GET",route="/health",le="10"} 2"#
        ));
        assert!(out.contains(
            r#"http_request_duration_seconds_bucket{method="GET",route="/health",le="+Inf"} 3"#
        ));
        assert!(
            out.contains(r#"http_request_duration_seconds_count{method="GET",route="/health"} 3"#)
        );
    }

    #[test]
    fn label_values_are_escaped() {
        assert_eq!(labels(&[("a", "x\"y\\z")]), r#"{a="x\"y\\z"}"#);
    }
}
