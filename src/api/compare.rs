//! `GET /recommendations/compare`: the weight sandbox in one request. The
//! same listener is ranked with the configured weights and with the weights
//! of the request, and every track of the second list says how it moved.
//! The bandit is left out, so both lists are reproducible.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;

use crate::api::dto::{ErrorBody, RecommendQuery, RecommendationDto, WeightsDto};
use crate::api::error::ApiError;
use crate::api::extract::ApiQuery;
use crate::api::feedback::drawn_weights;
use crate::api::handlers::{check_range, describe, overrides_of};
use crate::api::state::AppState;
use crate::recommender::WeightOverrides;

const MAX_TOP: usize = 100;

/// How a track's place changed against the default list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Movement {
    Same,
    Up {
        by: usize,
    },
    Down {
        by: usize,
    },
    /// Not in the default list at all.
    New,
}

/// Movement of every track of `next` against `base`, and how many tracks of
/// `base` are gone from `next`.
pub fn movements(base: &[u32], next: &[u32]) -> (Vec<Movement>, usize) {
    let moved = next
        .iter()
        .enumerate()
        .map(
            |(rank, track)| match base.iter().position(|other| other == track) {
                None => Movement::New,
                Some(old) if old > rank => Movement::Up { by: old - rank },
                Some(old) if old < rank => Movement::Down { by: rank - old },
                Some(_) => Movement::Same,
            },
        )
        .collect();
    let gone = base.iter().filter(|track| !next.contains(track)).count();
    (moved, gone)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComparedItemDto {
    #[serde(flatten)]
    pub item: RecommendationDto,
    pub movement: Movement,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComparisonResponse {
    pub user: u32,
    pub history_len: usize,
    /// The weights of the second list: the request's where given, else the configured.
    pub weights: WeightsDto,
    /// Ranked with the configured weights.
    pub base: Vec<RecommendationDto>,
    /// Ranked with the request's weights.
    pub custom: Vec<ComparedItemDto>,
    pub up: usize,
    pub down: usize,
    pub added: usize,
    /// Tracks of `base` missing from `custom`.
    pub gone: usize,
}

/// The default list and the list with the given weights, with the movement
/// of every track. Without weights both lists are the same.
#[utoipa::path(
    get,
    path = "/recommendations/compare",
    tag = "recommendations",
    params(RecommendQuery),
    responses(
        (status = 200, description = "Both lists and the movements", body = ComparisonResponse),
        (status = 400, description = "A parameter is out of range or missing", body = ErrorBody),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn compare(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RecommendQuery>,
) -> Result<Json<ComparisonResponse>, ApiError> {
    check_range("top", query.top, MAX_TOP)?;
    let overrides = overrides_of(&query)?;
    let Some(user) = state.store.user(query.user).await? else {
        return Err(ApiError::NotFound(format!("user {} not found", query.user)));
    };
    let history = state.store.history(query.user).await?;
    let history_len = history.len();

    let top = query.top;
    let (base, custom) = state
        .rank(move |recommender| {
            let country = user.country.as_deref();
            let base =
                recommender.recommend(history.clone(), country, top, WeightOverrides::default());
            let custom = recommender.recommend(history, country, top, overrides);
            (base, custom)
        })
        .await?;

    let base_tracks: Vec<u32> = base.iter().map(|item| item.track_index).collect();
    let custom_tracks: Vec<u32> = custom.iter().map(|item| item.track_index).collect();
    let (moved, gone) = movements(&base_tracks, &custom_tracks);
    let count =
        |wanted: fn(&Movement) -> bool| moved.iter().filter(|movement| wanted(movement)).count();

    Ok(Json(ComparisonResponse {
        user: query.user,
        history_len,
        weights: drawn_weights(overrides, state.config().weights),
        up: count(|movement| matches!(movement, Movement::Up { .. })),
        down: count(|movement| matches!(movement, Movement::Down { .. })),
        added: count(|movement| matches!(movement, Movement::New)),
        gone,
        base: describe(&state, base).await?,
        custom: describe(&state, custom)
            .await?
            .into_iter()
            .zip(moved)
            .map(|(item, movement)| ComparedItemDto { item, movement })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;
    use crate::api::router;
    use crate::api::test_support::{send, state};

    #[test]
    fn movements_follow_the_places() {
        let (moved, gone) = movements(&[1, 2, 3, 4], &[3, 1, 9, 2]);
        assert_eq!(
            moved,
            vec![
                Movement::Up { by: 2 },
                Movement::Down { by: 1 },
                Movement::New,
                Movement::Down { by: 2 },
            ]
        );
        assert_eq!(gone, 1);
        assert_eq!(movements(&[1], &[1]), (vec![Movement::Same], 0));
    }

    #[tokio::test]
    async fn both_lists_come_back_with_movements() {
        let app = router(state().await);
        let (status, body) = send(&app, "GET", "/recommendations/compare?user=2&cf=0", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["weights"]["cf"], 0.0);
        assert!(body["base"].is_array());
        let custom = body["custom"].as_array().unwrap();
        // the movement sits next to the track fields
        for item in custom {
            assert!(item["track_index"].is_number());
            assert!(item["movement"]["kind"].is_string());
        }

        let (_, same) = send(&app, "GET", "/recommendations/compare?user=1", None).await;
        assert_eq!(
            same["base"].as_array().unwrap().len(),
            same["custom"].as_array().unwrap().len()
        );
        assert_eq!(
            (same["up"].clone(), same["gone"].clone()),
            (0.into(), 0.into())
        );

        let (status, _) = send(&app, "GET", "/recommendations/compare?user=99", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
