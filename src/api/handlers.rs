use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{Duration, Utc};

use crate::api::dto::{
    ErrorBody, EventDto, HealthDto, NewEvent, RecommendQuery, RecommendationDto,
    RecommendationsResponse, TrackDto, TrackSearchQuery, UserDto, UserSummaryDto, UsersQuery,
    UsersResponse,
};
use crate::api::error::ApiError;
use crate::api::state::{AppState, ModelStatus};
use crate::domain::ListenEvent;
use crate::models::candidate::RecommendationContext;

const MAX_TOP: usize = 100;
const RECENT_TRACKS: usize = 10;
const MAX_USERS_PAGE: usize = 200;
const MAX_SEARCH_RESULTS: usize = 50;
const MIN_SEARCH_LEN: usize = 2;

fn check_range(name: &str, value: usize, max: usize) -> Result<(), ApiError> {
    if (1..=max).contains(&value) {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!(
            "{name} must be between 1 and {max}"
        )))
    }
}

/// Checks that the service is up and the database answers.
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "Service and database are up", body = HealthDto),
        (status = 503, description = "The database does not answer", body = ErrorBody),
    )
)]
pub async fn health(State(state): State<AppState>) -> Result<Json<HealthDto>, ApiError> {
    let counts = state.store.counts().await?;
    Ok(Json(HealthDto {
        status: "ok",
        users: counts.users,
        tracks: counts.tracks,
        events: counts.events,
        sessions: counts.sessions,
    }))
}

/// Recommends tracks for a user, with the reasons behind each one.
#[utoipa::path(
    get,
    path = "/recommendations",
    tag = "recommendations",
    params(RecommendQuery),
    responses(
        (status = 200, description = "Ranked tracks, best first", body = RecommendationsResponse),
        (status = 400, description = "`top` is out of range or `user` is missing", body = ErrorBody),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn recommendations(
    State(state): State<AppState>,
    Query(query): Query<RecommendQuery>,
) -> Result<Json<RecommendationsResponse>, ApiError> {
    check_range("top", query.top, MAX_TOP)?;

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

/// Records that a user listened to a track.
#[utoipa::path(
    post,
    path = "/events",
    tag = "events",
    request_body = NewEvent,
    responses(
        (status = 201, description = "The event was stored", body = EventDto),
        (status = 404, description = "Unknown user or track", body = ErrorBody),
    )
)]
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

/// Profile of a user with the tracks they listened to last.
#[utoipa::path(
    get,
    path = "/users/{user_index}",
    tag = "users",
    params(("user_index" = u32, Path, description = "Index of the user")),
    responses(
        (status = 200, description = "The user profile", body = UserDto),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
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

/// What the current models were trained on and whether a training is running.
#[utoipa::path(
    get,
    path = "/model",
    tag = "model",
    responses((status = 200, description = "Model status", body = ModelStatus))
)]
pub async fn model_status(State(state): State<AppState>) -> Json<ModelStatus> {
    Json(state.status())
}

/// Starts retraining in the background. Requests keep using the old models until it ends.
#[utoipa::path(
    post,
    path = "/model/retrain",
    tag = "model",
    responses(
        (status = 202, description = "Training started", body = ModelStatus),
        (status = 409, description = "Training is already running", body = ErrorBody),
    )
)]
pub async fn retrain(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ModelStatus>), ApiError> {
    if !state.retrain() {
        return Err(ApiError::Conflict("training is already running".to_string()));
    }
    Ok((StatusCode::ACCEPTED, Json(state.status())))
}

/// Lists users, the most active first. Used to pick who to recommend for.
#[utoipa::path(
    get,
    path = "/users",
    tag = "users",
    params(UsersQuery),
    responses(
        (status = 200, description = "One page of users", body = UsersResponse),
        (status = 400, description = "`limit` is out of range", body = ErrorBody),
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UsersQuery>,
) -> Result<Json<UsersResponse>, ApiError> {
    check_range("limit", query.limit, MAX_USERS_PAGE)?;

    let page = state.store.users_page(query.limit, query.offset).await?;
    Ok(Json(UsersResponse {
        total: page.total,
        items: page.items.into_iter().map(UserSummaryDto::from).collect(),
    }))
}

/// Finds tracks by a part of the title or of the artist name.
#[utoipa::path(
    get,
    path = "/tracks",
    tag = "tracks",
    params(TrackSearchQuery),
    responses(
        (status = 200, description = "Matching tracks ordered by artist and title", body = Vec<TrackDto>),
        (status = 400, description = "`q` is too short or `limit` is out of range", body = ErrorBody),
    )
)]
pub async fn search_tracks(
    State(state): State<AppState>,
    Query(query): Query<TrackSearchQuery>,
) -> Result<Json<Vec<TrackDto>>, ApiError> {
    let text = query.q.trim();
    if text.chars().count() < MIN_SEARCH_LEN {
        return Err(ApiError::BadRequest(format!(
            "q must have at least {MIN_SEARCH_LEN} characters"
        )));
    }
    check_range("limit", query.limit, MAX_SEARCH_RESULTS)?;

    let tracks = state.store.search_tracks(text, query.limit).await?;
    Ok(Json(tracks.into_iter().map(TrackDto::from).collect()))
}
