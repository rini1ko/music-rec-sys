use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{Duration, Utc};

use crate::api::dto::{
    ErrorBody, EventDto, HealthDto, NewEvent, NewUser, RecommendQuery, RecommendationDto,
    RecommendationsResponse, SimilarQuery, TrackDto, TrackSearchQuery, UserDto, UserSummaryDto,
    UsersQuery, UsersResponse,
};
use crate::api::error::ApiError;
use crate::api::extract::{ApiJson, ApiPath, ApiQuery};
use crate::api::feedback::drawn_weights;
use crate::api::rec_cache::CacheKey;
use crate::api::state::{AppState, ModelStatus};
use crate::domain::ListenEvent;
use crate::models::bandit::{arm_stats, sample_overrides};
use crate::models::online::session_tail;
use crate::ranker::Recommendation;
use crate::recommender::WeightOverrides;
use crate::rng::Rng;

const MAX_TOP: usize = 100;
const MAX_WEIGHT: f64 = 10.0;
const RECENT_TRACKS: usize = 10;
const MAX_USERS_PAGE: usize = 200;
const MAX_SEARCH_RESULTS: usize = 50;
const MIN_SEARCH_LEN: usize = 2;
const MAX_COUNTRY_LEN: usize = 56;
const MAX_GENDER_LEN: usize = 20;

pub(crate) fn check_range(name: &str, value: usize, max: usize) -> Result<(), ApiError> {
    if (1..=max).contains(&value) {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!(
            "{name} must be between 1 and {max}"
        )))
    }
}

fn check_weight(name: &str, value: Option<f64>) -> Result<Option<f64>, ApiError> {
    match value {
        Some(weight) if !(0.0..=MAX_WEIGHT).contains(&weight) => Err(ApiError::BadRequest(
            format!("{name} must be between 0 and {MAX_WEIGHT}"),
        )),
        other => Ok(other),
    }
}

pub(crate) fn overrides_of(query: &RecommendQuery) -> Result<WeightOverrides, ApiError> {
    Ok(WeightOverrides {
        trends: check_weight("trends", query.trends)?,
        markov: check_weight("markov", query.markov)?,
        co_listening: check_weight("co_listening", query.co_listening)?,
        cf: check_weight("cf", query.cf)?,
        mf: check_weight("mf", query.mf)?,
    })
}

/// Empty text counts as missing.
fn clean(name: &str, text: Option<String>, max: usize) -> Result<Option<String>, ApiError> {
    let Some(text) = text else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max {
        return Err(ApiError::BadRequest(format!(
            "{name} is longer than {max} characters"
        )));
    }
    Ok(Some(text.to_string()))
}

/// Adds the title and the artist to ranked tracks, with one query for all of them.
pub(crate) async fn describe(
    state: &AppState,
    ranked: Vec<Recommendation>,
) -> Result<Vec<RecommendationDto>, ApiError> {
    let wanted: Vec<u32> = ranked.iter().map(|item| item.track_index).collect();
    let mut tracks = state.store.tracks(&wanted).await?;
    Ok(ranked
        .into_iter()
        .map(|recommendation| {
            let track = tracks.remove(&recommendation.track_index);
            RecommendationDto::new(recommendation, track)
        })
        .collect())
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

/// Recommends tracks for a user, with the reasons behind each one. A user without
/// history gets what is popular in their country. The model weights can be
/// changed for one request to see how each model influences the result.
#[utoipa::path(
    get,
    path = "/recommendations",
    tag = "recommendations",
    params(RecommendQuery),
    responses(
        (status = 200, description = "Ranked tracks, best first", body = RecommendationsResponse),
        (status = 400, description = "A parameter is out of range or missing", body = ErrorBody),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn recommendations(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RecommendQuery>,
) -> Result<Json<RecommendationsResponse>, ApiError> {
    check_range("top", query.top, MAX_TOP)?;
    let mut overrides = overrides_of(&query)?;

    // with likes and skips, each request draws its own weights (R3); such
    // lists are not cached, or the first draw would stick
    let arms = if query.bandit {
        arm_stats(&state.store.feedback_stats(query.user).await?)
    } else {
        HashMap::new()
    };
    let key = CacheKey::new(query.user, query.top, &overrides);
    if arms.is_empty()
        && let Some(cached) = state.cache.get(&key)
    {
        return Ok(Json(cached));
    }
    let bandit_weights = (!arms.is_empty()).then(|| {
        let configured = state.config().weights;
        overrides = sample_overrides(configured, &arms, overrides, &mut Rng::from_time());
        drawn_weights(overrides, configured)
    });

    let Some(user) = state.store.user(query.user).await? else {
        return Err(ApiError::NotFound(format!("user {} not found", query.user)));
    };
    let history = state.store.history(query.user).await?;
    let history_len = history.len();

    let top = query.top;
    let ranked = state
        .rank(move |recommender| {
            recommender.recommend(history, user.country.as_deref(), top, overrides)
        })
        .await?;

    let response = RecommendationsResponse {
        user: query.user,
        history_len,
        items: describe(&state, ranked).await?,
        bandit_weights,
    };
    if response.bandit_weights.is_none() {
        state.cache.insert(key, response.clone());
    }
    Ok(Json(response))
}

/// Tracks that fit after the given one, with the reasons behind each.
#[utoipa::path(
    get,
    path = "/tracks/{track_index}/similar",
    tag = "tracks",
    params(
        ("track_index" = u32, Path, description = "Index of the track"),
        SimilarQuery,
    ),
    responses(
        (status = 200, description = "Ranked tracks, best first", body = Vec<RecommendationDto>),
        (status = 400, description = "`top` is out of range", body = ErrorBody),
        (status = 404, description = "No such track", body = ErrorBody),
    )
)]
pub async fn similar_tracks(
    State(state): State<AppState>,
    ApiPath(track_index): ApiPath<u32>,
    ApiQuery(query): ApiQuery<SimilarQuery>,
) -> Result<Json<Vec<RecommendationDto>>, ApiError> {
    check_range("top", query.top, MAX_TOP)?;
    if state.store.track(track_index).await?.is_none() {
        return Err(ApiError::NotFound(format!("track {track_index} not found")));
    }

    let top = query.top;
    let ranked = state
        .rank(move |recommender| {
            recommender.recommend(vec![track_index], None, top, WeightOverrides::default())
        })
        .await?;

    Ok(Json(describe(&state, ranked).await?))
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
    ApiJson(new): ApiJson<NewEvent>,
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

    // the two plays before this one decide which transitions it adds (R1, R2)
    let recent = state.store.recent_events(new.user, 2).await?;

    // unknown users and tracks come back as 404 from the store
    let event = ListenEvent {
        user_index: new.user,
        track_index: new.track,
        timestamp,
    };
    state.store.add_events(vec![event]).await?;

    let previous = session_tail(&recent, timestamp, state.config().session_gap);
    state.record_play(&previous, new.track);
    // the history changed, so the lists ranked from the old one are wrong
    state.cache.forget_user(new.user);

    let created = EventDto {
        user: new.user,
        track: new.track,
        timestamp,
    };
    // nobody listening is not an error: the play is stored either way
    let _ = state.plays.send(created.clone());
    Ok((StatusCode::CREATED, Json(created)))
}

/// Creates a listener without history. Their first recommendations are popular tracks.
#[utoipa::path(
    post,
    path = "/users",
    tag = "users",
    request_body = NewUser,
    responses(
        (status = 201, description = "The new user", body = UserDto),
        (status = 400, description = "A field is invalid", body = ErrorBody),
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewUser>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let country = clean("country", new.country, MAX_COUNTRY_LEN)?;
    let gender = clean("gender", new.gender, MAX_GENDER_LEN)?;
    if let Some(age) = new.age
        && !(1.0..=120.0).contains(&age)
    {
        return Err(ApiError::BadRequest(
            "age must be between 1 and 120".to_string(),
        ));
    }

    let user = state.store.create_user(country, new.age, gender).await?;
    let created = UserDto {
        user_index: user.user_index,
        gender: user.gender,
        age: user.age,
        country: user.country,
        history_len: 0,
        recent: Vec::new(),
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
    ApiPath(user_index): ApiPath<u32>,
) -> Result<Json<UserDto>, ApiError> {
    let Some(user) = state.store.user(user_index).await? else {
        return Err(ApiError::NotFound(format!("user {user_index} not found")));
    };
    let history = state.store.history(user_index).await?;

    let newest: Vec<u32> = history.iter().rev().take(RECENT_TRACKS).copied().collect();
    let tracks = state.store.tracks(&newest).await?;
    // a track can repeat among the newest plays, so look each one up instead of draining
    let recent = newest
        .iter()
        .filter_map(|track_index| tracks.get(track_index).cloned())
        .map(TrackDto::from)
        .collect();

    Ok(Json(UserDto {
        user_index,
        gender: user.gender,
        age: user.age,
        country: user.country,
        history_len: history.len(),
        recent,
    }))
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
    ApiQuery(query): ApiQuery<UsersQuery>,
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
    ApiQuery(query): ApiQuery<TrackSearchQuery>,
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
        return Err(ApiError::Conflict(
            "training is already running".to_string(),
        ));
    }
    Ok((StatusCode::ACCEPTED, Json(state.status())))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::api::router;
    use crate::api::test_support::{send, state};

    fn tracks(body: &serde_json::Value) -> Vec<u64> {
        body.as_array()
            .unwrap()
            .iter()
            .map(|item| item["track_index"].as_u64().unwrap())
            .collect()
    }

    #[tokio::test]
    async fn a_play_teaches_the_next_track_model_without_training() {
        let state = state().await;
        let app = router(state.clone());
        let (_, before) = send(&app, "GET", "/tracks/11/similar", None).await;
        assert!(!tracks(&before).contains(&13));

        // user 1 played 10 and 11 a minute apart; 13 now follows 11 in the same session
        let (status, _) = send(&app, "POST", "/events", Some(r#"{"user":1,"track":13}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(state.online_events(), 1);

        let (_, after) = send(&app, "GET", "/tracks/11/similar", None).await;
        let item = after
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["track_index"] == 13)
            .expect("13 follows 11 now");
        assert!(
            item["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason["source"] == "markov")
        );
    }

    #[tokio::test]
    async fn matrix_factorization_can_be_switched_on_for_one_request() {
        let app = router(state().await);
        let uri = "/recommendations?user=1&trends=0&markov=0&co_listening=0&cf=0&mf=1";
        let (status, body) = send(&app, "GET", uri, None).await;
        assert_eq!(status, StatusCode::OK);
        // user 2 played 10, 11 and 12 together, so 12 fits the taste of user 1
        assert_eq!(body["items"][0]["track_index"], 12);
        assert_eq!(body["items"][0]["reasons"][0]["source"], "mf");

        let (status, _) = send(&app, "GET", "/recommendations?user=1&mf=11", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn retraining_starts_the_online_counts_again() {
        let state = state().await;
        let app = router(state.clone());
        send(&app, "POST", "/events", Some(r#"{"user":1,"track":13}"#)).await;
        assert_eq!(state.online_events(), 1);

        assert!(state.retrain());
        for _ in 0..100 {
            if !state.status().training {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        // the play is in the trained models now, so it is not counted twice
        assert_eq!(state.online_events(), 0);
    }
}
