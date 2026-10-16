//! Likes and skips (R3). A feedback is credited to the model that did most to
//! put the track on the listener's list; the bandit in `/recommendations`
//! then leans towards the models the listener likes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::dto::{ErrorBody, WeightsDto};
use crate::api::error::ApiError;
use crate::api::extract::{ApiJson, ApiPath, ApiQuery};
use crate::api::handlers::check_range;
use crate::api::state::AppState;
use crate::domain::{Feedback, FeedbackKind};
use crate::engine::Weights;
use crate::models::bandit::{ArmStats, arm_stats, credited_source};
use crate::recommender::WeightOverrides;

const MAX_PAGE: usize = 200;
/// How deep the listener's list is searched for the track that gets feedback.
const CREDIT_DEPTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackKindDto {
    Like,
    Skip,
}

impl From<FeedbackKindDto> for FeedbackKind {
    fn from(kind: FeedbackKindDto) -> Self {
        match kind {
            FeedbackKindDto::Like => FeedbackKind::Like,
            FeedbackKindDto::Skip => FeedbackKind::Skip,
        }
    }
}

impl From<FeedbackKind> for FeedbackKindDto {
    fn from(kind: FeedbackKind) -> Self {
        match kind {
            FeedbackKind::Like => FeedbackKindDto::Like,
            FeedbackKind::Skip => FeedbackKindDto::Skip,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFeedback {
    pub user: u32,
    pub track: u32,
    pub kind: FeedbackKindDto,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FeedbackDto {
    pub user: u32,
    pub track: u32,
    pub kind: FeedbackKindDto,
    /// The model credited with the track; `null` if no model proposed it.
    pub source: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FeedbackItemDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub kind: FeedbackKindDto,
    pub source: Option<String>,
    pub created_at: NaiveDateTime,
}

/// Likes and skips one model got, and the share of likes the bandit expects.
#[derive(Debug, Serialize, ToSchema)]
pub struct ArmDto {
    pub source: String,
    pub likes: u32,
    pub skips: u32,
    /// `(1 + likes) / (2 + likes + skips)`.
    pub expected_like_share: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FeedbackResponse {
    pub user: u32,
    /// Newest first.
    pub items: Vec<FeedbackItemDto>,
    pub models: Vec<ArmDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FeedbackQuery {
    /// How many feedback items to return, from 1 to 200. Default is 50.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// The weights a bandit draw produced, for the response.
pub(crate) fn drawn_weights(drawn: WeightOverrides, configured: Weights) -> WeightsDto {
    WeightsDto {
        trends: drawn.trends.unwrap_or(configured.trends),
        markov: drawn.markov.unwrap_or(configured.markov),
        co_listening: drawn.co_listening.unwrap_or(configured.co_listening),
        cf: drawn.cf.unwrap_or(configured.cf),
        mf: drawn.mf.unwrap_or(configured.mf),
    }
}

/// Records a like or a skip and credits it to the model behind the track.
#[utoipa::path(
    post,
    path = "/feedback",
    tag = "feedback",
    request_body = NewFeedback,
    responses(
        (status = 201, description = "The feedback was stored", body = FeedbackDto),
        (status = 401, description = "A valid x-api-key is required", body = ErrorBody),
        (status = 404, description = "Unknown user or track", body = ErrorBody),
        (status = 422, description = "`kind` is not like or skip", body = ErrorBody),
    )
)]
pub async fn add_feedback(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewFeedback>,
) -> Result<(StatusCode, Json<FeedbackDto>), ApiError> {
    let Some(user) = state.store.user(new.user).await? else {
        return Err(ApiError::NotFound(format!("user {} not found", new.user)));
    };
    let history = state.store.history(new.user).await?;

    // rank again with the configured weights to see who proposed the track
    let track = new.track;
    let source = state
        .rank(move |recommender| {
            recommender
                .recommend(
                    history,
                    user.country.as_deref(),
                    CREDIT_DEPTH,
                    WeightOverrides::default(),
                )
                .into_iter()
                .find(|item| item.track_index == track)
                .and_then(|item| credited_source(&item.reasons))
        })
        .await?;

    let feedback = Feedback {
        user_index: new.user,
        track_index: new.track,
        kind: new.kind.into(),
        source: source.map(|source| source.name().to_string()),
        created_at: Utc::now().naive_utc(),
    };
    state.store.add_feedback(feedback.clone()).await?;
    // the next list of this user is drawn with the new counts
    state.cache.forget_user(new.user);

    let created = FeedbackDto {
        user: feedback.user_index,
        track: feedback.track_index,
        kind: new.kind,
        source: feedback.source,
        created_at: feedback.created_at,
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// The listener's latest likes and skips and what the bandit learned from them.
#[utoipa::path(
    get,
    path = "/users/{user_index}/feedback",
    tag = "feedback",
    params(
        ("user_index" = u32, Path, description = "Index of the user"),
        FeedbackQuery,
    ),
    responses(
        (status = 200, description = "Feedback and per-model counts", body = FeedbackResponse),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn user_feedback(
    State(state): State<AppState>,
    ApiPath(user_index): ApiPath<u32>,
    ApiQuery(query): ApiQuery<FeedbackQuery>,
) -> Result<Json<FeedbackResponse>, ApiError> {
    check_range("limit", query.limit, MAX_PAGE)?;
    if state.store.user(user_index).await?.is_none() {
        return Err(ApiError::NotFound(format!("user {user_index} not found")));
    }
    let entries = state.store.feedback(user_index, query.limit).await?;
    let stats = state.store.feedback_stats(user_index).await?;

    let mut models: Vec<ArmDto> = arm_stats(&stats)
        .into_iter()
        .map(|(source, arm)| ArmDto {
            source: source.name().to_string(),
            likes: arm.likes,
            skips: arm.skips,
            expected_like_share: ArmStats::mean(arm),
        })
        .collect();
    models.sort_by(|a, b| a.source.cmp(&b.source));

    let items = entries
        .into_iter()
        .map(|entry| FeedbackItemDto {
            track_index: entry.track.track_index,
            title: entry.track.track_name,
            artist: entry.track.artist_name,
            kind: entry.feedback.kind.into(),
            source: entry.feedback.source,
            created_at: entry.feedback.created_at,
        })
        .collect();
    Ok(Json(FeedbackResponse {
        user: user_index,
        items,
        models,
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::api::router;
    use crate::api::test_support::{send, state};

    #[tokio::test]
    async fn feedback_is_credited_and_turns_the_bandit_on() {
        let app = router(state().await);
        // user 1 played 10 and 11; 12 is on their list thanks to the models
        let (status, body) = send(
            &app,
            "POST",
            "/feedback",
            Some(r#"{"user":1,"track":12,"kind":"like"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let credited = body["source"]
            .as_str()
            .expect("a model proposed 12")
            .to_string();

        let (status, body) = send(&app, "GET", "/users/1/feedback", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"][0]["kind"], "like");
        assert_eq!(body["models"][0]["source"], credited);
        assert_eq!(body["models"][0]["likes"], 1);

        // with feedback, the weights are drawn for every request
        let (_, list) = send(&app, "GET", "/recommendations?user=1", None).await;
        assert!(list["bandit_weights"].is_object());
        let (_, plain) = send(&app, "GET", "/recommendations?user=1&bandit=false", None).await;
        assert!(plain["bandit_weights"].is_null());
    }

    #[tokio::test]
    async fn a_track_no_model_proposed_has_no_source() {
        let app = router(state().await);
        // 13 was never played, so no model knows it
        let (status, body) = send(
            &app,
            "POST",
            "/feedback",
            Some(r#"{"user":1,"track":13,"kind":"skip"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(body["source"].is_null());

        let (_, list) = send(&app, "GET", "/recommendations?user=1", None).await;
        assert!(list["bandit_weights"].is_null());
    }

    #[tokio::test]
    async fn bad_feedback_is_rejected() {
        let app = router(state().await);
        let (status, _) = send(
            &app,
            "POST",
            "/feedback",
            Some(r#"{"user":1,"track":12,"kind":"love"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = send(
            &app,
            "POST",
            "/feedback",
            Some(r#"{"user":9,"track":12,"kind":"like"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&app, "GET", "/users/9/feedback", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
