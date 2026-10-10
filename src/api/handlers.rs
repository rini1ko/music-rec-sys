use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::{Value, json};

use crate::api::dto::{
    EventDto, NewEvent, RecommendQuery, RecommendationDto, RecommendationsResponse, TrackDto,
    UserDto,
};
use crate::api::error::ApiError;
use crate::api::state::AppState;
use crate::domain::ListenEvent;
use crate::models::candidate::RecommendationContext;

const RECENT_TRACKS: usize = 10;
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

pub async fn add_event(
    State(state): State<AppState>,
    Json(new): Json<NewEvent>,
) -> Result<(StatusCode, Json<EventDto>), ApiError> {
    let timestamp = match new.timestamp {
        Some(timestamp) => timestamp,
        None => state
            .store
            .latest_event_time()
            .await?
            .map(|latest| latest + Duration::minutes(1))
            .unwrap_or_else(|| Utc::now().naive_utc()),
    };

    // unknown users and tracks come back as 404 from the store
    let event = ListenEvent {
        user_index: new.user,
        track_index: new.track,
        timestamp,
    };
    state.store.add_events(vec![event]).await?;

    let created = EventDto {
        user: new.user,
        track: new.track,
        timestamp,
    };
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn user(
    State(state): State<AppState>,
    Path(user_index): Path<u32>,
) -> Result<Json<UserDto>, ApiError> {
    let Some(user) = state.store.user(user_index).await? else {
        return Err(ApiError::NotFound(format!("user {user_index} not found")));
    };
    let history = state.store.history(user_index).await?;

    let mut recent = Vec::new();
    for &track_index in history.iter().rev().take(RECENT_TRACKS) {
        if let Some(track) = state.store.track(track_index).await? {
            recent.push(TrackDto::from(track));
        }
    }

    Ok(Json(UserDto {
        user_index,
        gender: user.gender,
        age: user.age,
        country: user.country,
        history_len: history.len(),
        recent,
    }))
}
