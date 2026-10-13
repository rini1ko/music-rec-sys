//! Model evaluation from the API (B1). Works like retraining: it runs in the
//! background on all stored events, one at a time, and the result stays in
//! the app state until the next run.

use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use tokio::task::JoinError;
use utoipa::ToSchema;

use crate::api::dto::ErrorBody;
use crate::api::error::ApiError;
use crate::api::state::AppState;
use crate::db::error::DbError;
use crate::db::store::Store;
use crate::engine::EngineConfig;
use crate::evaluation::compare::{ModelScore, compare_models};
use crate::evaluation::{EvalConfig, EvalError, Evaluator};

const LOCK: &str = "the evaluation lock is never poisoned";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelScoreDto {
    /// `trends`, `markov`, `markov_2`, `co_listening`, `cf`, `hybrid` or `hybrid_rules`.
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

    /// Starts an evaluation in the background. False means one is already running.
    pub fn evaluate(&self) -> bool {
        {
            let mut status = self.evaluation.lock().expect(LOCK);
            if status.running {
                return false;
            }
            status.running = true;
            status.started_at = Some(Utc::now());
        }
        let state = self.clone();
        tokio::spawn(async move { state.run_evaluation().await });
        true
    }

    async fn run_evaluation(&self) {
        let started = Instant::now();
        let result = evaluate_models(&self.store, self.config().clone()).await;

        let mut status = self.evaluation.lock().expect(LOCK);
        status.running = false;
        status.finished_at = Some(Utc::now());
        status.seconds = Some(started.elapsed().as_secs_f32());
        match result {
            Ok(scores) => {
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

/// Trains on the older 80% of the events and judges the newer 20%, on a blocking thread.
async fn evaluate_models(
    store: &Store,
    config: EngineConfig,
) -> Result<Vec<ModelScore>, EvaluationError> {
    let events = store.all_events().await?;
    let tracks = store.all_tracks().await?;
    let users = store.all_users().await?;
    let scores = tokio::task::spawn_blocking(move || {
        let eval = EvalConfig {
            session_gap: config.session_gap,
            ..EvalConfig::default()
        };
        let mut evaluator = Evaluator::new(&events, &eval, &config)?.with_catalog(&tracks, &users);
        Ok::<_, EvalError>(compare_models(&mut evaluator, config.weights))
    })
    .await??;
    Ok(scores)
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
    responses(
        (status = 202, description = "Evaluation started", body = EvaluationStatus),
        (status = 409, description = "An evaluation is already running", body = ErrorBody),
    )
)]
pub async fn start_evaluation(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<EvaluationStatus>), ApiError> {
    if !state.evaluate() {
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
        assert!(state.evaluate());
        assert!(!state.evaluate());
    }

    #[tokio::test]
    async fn evaluation_reports_every_model_and_the_hybrid() {
        let app = router(state().await);
        let (status, body) = send(&app, "POST", "/model/evaluate", None).await;
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

        assert_eq!(report["running"], false);
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
    async fn evaluation_is_empty_before_the_first_run() {
        let app = router(state().await);
        let (status, body) = send(&app, "GET", "/model/evaluation", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["running"], false);
        assert!(body["models"].as_array().unwrap().is_empty());
    }
}
