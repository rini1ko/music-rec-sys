use axum::Json;
use axum::extract::{Query, State};
use serde_json::{Value, json};

use crate::api::dto::{RecommendQuery, RecommendationDto, RecommendationsResponse};
use crate::api::error::ApiError;
use crate::api::state::AppState;
use crate::models::candidate::RecommendationContext;

const MAX_TOP: usize = 100;

/// Also proves that the database answers.
pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let counts = state.store.counts().await?;
    Ok(Json(json!({
        "status": "ok",
        "users": counts.users,
        "tracks": counts.tracks,
        "events": counts.events,
        "sessions": counts.sessions,
    })))
}

pub async fn recommendations(
    State(state): State<AppState>,
    Query(query): Query<RecommendQuery>,
) -> Result<Json<RecommendationsResponse>, ApiError> {
    if !(1..=MAX_TOP).contains(&query.top) {
        return Err(ApiError::BadRequest(format!(
            "top must be between 1 and {MAX_TOP}"
        )));
    }

    let history = state.store.history(query.user).await?;
    let history_len = history.len();

    // ranking is CPU work, keep it off the async threads
    let ranker = state.ranker.clone();
    let top = query.top;
    let ranked = tokio::task::spawn_blocking(move || {
        let ranker = ranker.read().expect("the ranker lock is never poisoned");
        ranker.recommend(&RecommendationContext { history }, top)
    })
    .await?;

    let mut items = Vec::with_capacity(ranked.len());
    for recommendation in ranked {
        let track = state.store.track(recommendation.track_index).await?;
        items.push(RecommendationDto::new(recommendation, track));
    }

    Ok(Json(RecommendationsResponse {
        user: query.user,
        history_len,
        items,
    }))
}