//! Model evaluation from the API (B1). Works like retraining: it runs in the
//! background on all stored events, one at a time, and the result stays in
//! the app state until the next run. With `windows` it also judges the
//! models on consecutive time windows (W1).

use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinError;
use utoipa::{IntoParams, ToSchema};

use crate::api::dto::ErrorBody;
use crate::api::error::ApiError;
use crate::api::extract::ApiQuery;
use crate::api::handlers::check_range;
use crate::api::state::AppState;
use crate::db::error::DbError;
use crate::db::store::Store;
use crate::engine::EngineConfig;
use crate::evaluation::compare::{ModelScore, compare_models};
use crate::evaluation::windows::{self, Backtest, Stability, WindowConfig, WindowScores};
use crate::evaluation::{EvalConfig, EvalError, Evaluator};

const LOCK: &str = "the evaluation lock is never poisoned";
/// Each window trains every model again, so their number is kept small.
const MAX_WINDOWS: usize = 12;
const MAX_WINDOW_DAYS: usize = 90;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EvaluateQuery {
    /// Also judge the models on this many consecutive time windows, from 1 to
    /// 12. Default is 0: only the usual 80/20 split.
    #[serde(default)]
    pub windows: usize,
    /// Length of one window in days, from 1 to 90. Default is 7.
    #[serde(default = "default_window_days")]
    pub window_days: usize,
}

fn default_window_days() -> usize {
    7
}

/// One time window: trained on the plays up to `from`, judged on the plays after it up to `to`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WindowDto {
    pub from: NaiveDateTime,
    pub to: NaiveDateTime,
    pub train_events: usize,
    pub test_events: usize,
    /// Users with history and new tracks in the window.
    pub users: usize,
    /// Empty when the window could not be judged.
    pub models: Vec<ModelScoreDto>,
    /// The models with the highest NDCG of this window.
    pub best: Vec<&'static str>,
}

impl From<&WindowScores> for WindowDto {
    fn from(window: &WindowScores) -> Self {
        Self {
            from: window.from,
            to: window.to,
            train_events: window.train_events,
            test_events: window.test_events,
            users: window.users,
            models: window.scores.iter().map(ModelScoreDto::from).collect(),
            best: window.best(),
        }
    }
}

/// How steady one model is across the windows.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StabilityDto {
    pub model: &'static str,
    /// Windows the model was judged in.
    pub windows: usize,
    pub mean_ndcg: f64,
    /// Standard deviation of the NDCG; lower is steadier.
    pub std_ndcg: f64,
    pub min_ndcg: f64,
    pub max_ndcg: f64,
    pub mean_hit_rate: f64,
    /// Change of the NDCG per window; below 0 the model does worse on newer plays.
    pub trend: f64,
    /// Windows where the model had the highest NDCG.
    pub wins: usize,
}

impl From<&Stability> for StabilityDto {
    fn from(row: &Stability) -> Self {
        Self {
            model: row.model,
            windows: row.windows,
            mean_ndcg: row.mean_ndcg,
            std_ndcg: row.std_ndcg,
            min_ndcg: row.min_ndcg,
            max_ndcg: row.max_ndcg,
            mean_hit_rate: row.mean_hit_rate,
            trend: row.trend,
            wins: row.wins,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelScoreDto {
    /// `trends`, `markov`, `markov_2`, `co_listening`, `cf`, `mf`, `hybrid` or `hybrid_rules`.
    pub model: &'static str,
    pub ndcg: f64,
    pub hit_rate: f64,
    pub precision: f64,
    pub recall: f64,
    /// Share of the catalog shown to at least one user.
    pub coverage: f64,
    /// Mean self-information of the shown tracks, in bits.
    pub novelty: f64,
    /// Mean share of different artists in one list.
    pub diversity: Option<f64>,
}

impl From<&ModelScore> for ModelScoreDto {
    fn from(score: &ModelScore) -> Self {
        Self {
            model: score.model,
            ndcg: score.report.ndcg,
            hit_rate: score.report.hit_rate,
            precision: score.report.precision,
            recall: score.report.recall,
            coverage: score.report.coverage,
            novelty: score.report.novelty,
            diversity: score.report.diversity,
        }
    }
}

/// The last evaluation and whether one is running now.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct EvaluationStatus {
    pub running: bool,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub seconds: Option<f32>,
    /// Length of the judged list (NDCG@k, hit rate@k).
    pub k: usize,
    pub train_events: usize,
    pub test_events: usize,
    /// Users with new tracks in the test part, the ones the metrics average over.
    pub users: usize,
    pub last_error: Option<String>,
    /// Each model alone, then the hybrid. Empty until the first run ends.
    pub models: Vec<ModelScoreDto>,
    /// Length of one time window in days; `null` when the last run had no windows.
    pub window_days: Option<usize>,
    /// The time windows of the last run, oldest first.
    pub windows: Vec<WindowDto>,
    /// Each model over all the judged windows.
    pub stability: Vec<StabilityDto>,
}

#[derive(Debug, Error)]
enum EvaluationError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Eval(#[from] EvalError),
    #[error("evaluation task failed: {0}")]
    Task(#[from] JoinError),
}

impl AppState {
    /// The last evaluation and whether one is running now.
    pub fn evaluation(&self) -> EvaluationStatus {
        self.evaluation.lock().expect(LOCK).clone()
    }

    /// Starts an evaluation in the background, on time windows too when
    /// `windows` is given. False means one is already running.
    pub fn evaluate(&self, windows: Option<WindowConfig>) -> bool {
        {
            let mut status = self.evaluation.lock().expect(LOCK);
            if status.running {
                return false;
            }
            status.running = true;
            status.started_at = Some(Utc::now());
        }
        let state = self.clone();
        tokio::spawn(async move { state.run_evaluation(windows).await });
        true
    }

    async fn run_evaluation(&self, windows: Option<WindowConfig>) {
        let started = Instant::now();
        let days = windows.as_ref().map(|config| config.length.num_days());
        let result = evaluate_models(&self.store, self.config().clone(), windows).await;

        let mut status = self.evaluation.lock().expect(LOCK);
        status.running = false;
        status.finished_at = Some(Utc::now());
        status.seconds = Some(started.elapsed().as_secs_f32());
        match result {
            Ok((scores, backtest)) => {
                let backtest = backtest.unwrap_or(Backtest {
                    windows: Vec::new(),
                    stability: Vec::new(),
                });
                status.window_days = days.and_then(|days| usize::try_from(days).ok());
                status.windows = backtest.windows.iter().map(WindowDto::from).collect();
                status.stability = backtest.stability.iter().map(StabilityDto::from).collect();
                if let Some(first) = scores.first() {
                    status.k = first.report.k;
                    status.train_events = first.report.train_events;
                    status.test_events = first.report.test_events;
                    status.users = first.report.users_evaluated;
                }
                status.models = scores.iter().map(ModelScoreDto::from).collect();
                status.last_error = None;
                tracing::info!(
                    seconds = status.seconds,
                    users = status.users,
                    "models evaluated"
                );
            }
            Err(error) => {
                tracing::error!(%error, "evaluation failed");
                status.last_error = Some(error.to_string());
            }
        }
    }
}

/// Trains on the older 80% of the events and judges the newer 20%, then on
/// the time windows if asked, all on a blocking thread.
async fn evaluate_models(
    store: &Store,
    config: EngineConfig,
    windows: Option<WindowConfig>,
) -> Result<(Vec<ModelScore>, Option<Backtest>), EvaluationError> {
    let events = store.all_events().await?;
    let tracks = store.all_tracks().await?;
    let users = store.all_users().await?;
    let result = tokio::task::spawn_blocking(move || {
        let eval = EvalConfig {
            session_gap: config.session_gap,
            ..EvalConfig::default()
        };
        let mut evaluator = Evaluator::new(&events, &eval, &config)?.with_catalog(&tracks, &users);
        let scores = compare_models(&mut evaluator, config.weights);
        let backtest = windows
            .map(|windows| windows::backtest(&events, Some((&tracks, &users)), &windows, &config))
            .transpose()?;
        Ok::<_, EvalError>((scores, backtest))
    })
    .await??;
    Ok(result)
}

/// Checks the query and turns it into window settings; `None` without windows.
fn window_config(
    query: &EvaluateQuery,
    config: &EngineConfig,
) -> Result<Option<WindowConfig>, ApiError> {
    if query.windows == 0 {
        return Ok(None);
    }
    check_range("windows", query.windows, MAX_WINDOWS)?;
    check_range("window_days", query.window_days, MAX_WINDOW_DAYS)?;
    Ok(Some(WindowConfig {
        windows: query.windows,
        // checked above: at most 90 days
        length: Duration::days(query.window_days as i64),
        session_gap: config.session_gap,
        ..WindowConfig::default()
    }))
}

/// NDCG and hit rate of each model alone and of the hybrid, from the last run.
#[utoipa::path(
    get,
    path = "/model/evaluation",
    tag = "model",
    responses((status = 200, description = "The last evaluation", body = EvaluationStatus))
)]
pub async fn evaluation(State(state): State<AppState>) -> Json<EvaluationStatus> {
    Json(state.evaluation())
}

/// Starts an evaluation in the background. Poll `GET /model/evaluation` for the result.
#[utoipa::path(
    post,
    path = "/model/evaluate",
    tag = "model",
    params(EvaluateQuery),
    responses(
        (status = 202, description = "Evaluation started", body = EvaluationStatus),
        (status = 400, description = "`windows` or `window_days` is out of range", body = ErrorBody),
        (status = 409, description = "An evaluation is already running", body = ErrorBody),
    )
)]
pub async fn start_evaluation(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<EvaluateQuery>,
) -> Result<(StatusCode, Json<EvaluationStatus>), ApiError> {
    let windows = window_config(&query, state.config())?;
    if !state.evaluate(windows) {
        return Err(ApiError::Conflict(
            "an evaluation is already running".to_string(),
        ));
    }
    Ok((StatusCode::ACCEPTED, Json(state.evaluation())))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::api::router;
    use crate::api::test_support::{send, state};

    #[tokio::test]
    async fn only_one_evaluation_runs_at_a_time() {
        let state = state().await;
        assert!(state.evaluate(None));
        assert!(!state.evaluate(None));
    }

    /// Starts an evaluation with `query` and waits until it ends.
    async fn finished(query: &str) -> serde_json::Value {
        let app = router(state().await);
        let uri = format!("/model/evaluate{query}");
        let (status, body) = send(&app, "POST", &uri, None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["running"], true);

        let mut report = body;
        for _ in 0..100 {
            let (_, current) = send(&app, "GET", "/model/evaluation", None).await;
            report = current;
            if report["running"] == false {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        report
    }

    #[tokio::test]
    async fn evaluation_reports_every_model_and_the_hybrid() {
        let report = finished("").await;
        assert_eq!(report["running"], false);
        assert_eq!(report["window_days"], serde_json::Value::Null);
        assert!(report["windows"].as_array().unwrap().is_empty());
        assert_eq!(report["last_error"], serde_json::Value::Null);
        let models: Vec<&str> = report["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["model"].as_str().unwrap())
            .collect();
        assert_eq!(
            models,
            [
                "trends",
                "markov",
                "markov_2",
                "co_listening",
                "cf",
                "mf",
                "hybrid",
                "hybrid_rules"
            ]
        );
    }

    #[tokio::test]
    async fn evaluation_can_walk_through_time_windows() {
        // the test plays are minutes apart, one-day windows hold all of them
        let report = finished("?windows=2&window_days=1").await;
        assert_eq!(report["last_error"], serde_json::Value::Null);
        assert_eq!(report["window_days"], 1);

        let windows = report["windows"].as_array().unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0]["to"], windows[1]["from"]);
        // the older window has no plays before it to train on
        assert_eq!(windows[0]["train_events"], 0);
        assert!(windows[0]["models"].as_array().unwrap().is_empty());
        let plays: u64 = windows
            .iter()
            .map(|window| window["test_events"].as_u64().unwrap())
            .sum();
        assert_eq!(plays, 5);
    }

    #[tokio::test]
    async fn window_settings_out_of_range_are_rejected() {
        let app = router(state().await);
        for query in [
            "?windows=13",
            "?windows=2&window_days=0",
            "?windows=2&window_days=91",
        ] {
            let uri = format!("/model/evaluate{query}");
            let (status, body) = send(&app, "POST", &uri, None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
            assert!(body["error"].as_str().unwrap().contains("must be between"));
        }
        // nothing was started by the rejected requests
        let (_, body) = send(&app, "GET", "/model/evaluation", None).await;
        assert_eq!(body["running"], false);
    }

    #[tokio::test]
    async fn evaluation_is_empty_before_the_first_run() {
        let app = router(state().await);
        let (status, body) = send(&app, "GET", "/model/evaluation", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["running"], false);
        assert!(body["models"].as_array().unwrap().is_empty());
    }
}
