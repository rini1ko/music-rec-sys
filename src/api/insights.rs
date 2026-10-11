//! Endpoints that look at the stored data rather than rank it: statistics for
//! the dashboard (E1), artists (E2) and a listener's history (E3). The radio
//! (E4) lives here too: it is a playlist built for one listener.

use axum::Json;
use axum::extract::{Path, Query, State};

use crate::api::dto::{
    ArtistDto, ArtistProfileDto, ArtistQuery, ArtistSearchQuery, ErrorBody, HistoryQuery,
    HistoryResponse, OverviewDto, OverviewQuery, RadioQuery, RadioResponse, TrackDto,
};
use crate::api::error::ApiError;
use crate::api::handlers::{check_range, describe};
use crate::api::state::AppState;

const MAX_DAYS: u32 = 365;
const MAX_TOP_LIST: usize = 50;
const MAX_ARTISTS: usize = 50;
const MAX_ARTIST_TRACKS: usize = 100;
const MIN_SEARCH_LEN: usize = 2;
const MAX_HISTORY_PAGE: usize = 200;
const MAX_RADIO_LENGTH: usize = 50;

/// Totals, top tracks and artists, plays by day, hour and weekday, and
/// countries for the last days of the data.
#[utoipa::path(
    get,
    path = "/stats/overview",
    tag = "stats",
    params(OverviewQuery),
    responses(
        (status = 200, description = "Statistics of the window", body = OverviewDto),
        (status = 400, description = "`days` or `top` is out of range", body = ErrorBody),
    )
)]
pub async fn overview(
    State(state): State<AppState>,
    Query(query): Query<OverviewQuery>,
) -> Result<Json<OverviewDto>, ApiError> {
    if !(1..=MAX_DAYS).contains(&query.days) {
        return Err(ApiError::BadRequest(format!(
            "days must be between 1 and {MAX_DAYS}"
        )));
    }
    check_range("top", query.top, MAX_TOP_LIST)?;

    let overview = state.store.overview(query.days, query.top).await?;
    Ok(Json(OverviewDto::new(query.days, overview)))
}

/// Finds artists by a part of their name, the most played first.
#[utoipa::path(
    get,
    path = "/artists",
    tag = "artists",
    params(ArtistSearchQuery),
    responses(
        (status = 200, description = "Matching artists", body = Vec<ArtistDto>),
        (status = 400, description = "`q` is too short or `limit` is out of range", body = ErrorBody),
    )
)]
pub async fn search_artists(
    State(state): State<AppState>,
    Query(query): Query<ArtistSearchQuery>,
) -> Result<Json<Vec<ArtistDto>>, ApiError> {
    let text = query.q.trim();
    if text.chars().count() < MIN_SEARCH_LEN {
        return Err(ApiError::BadRequest(format!(
            "q must have at least {MIN_SEARCH_LEN} characters"
        )));
    }
    check_range("limit", query.limit, MAX_ARTISTS)?;

    let artists = state.store.search_artists(text, query.limit).await?;
    Ok(Json(artists.into_iter().map(ArtistDto::from).collect()))
}

/// An artist with play counts and their most played tracks.
#[utoipa::path(
    get,
    path = "/artists/{artist_index}",
    tag = "artists",
    params(
        ("artist_index" = u32, Path, description = "Index of the artist"),
        ArtistQuery,
    ),
    responses(
        (status = 200, description = "The artist", body = ArtistProfileDto),
        (status = 400, description = "`top` is out of range", body = ErrorBody),
        (status = 404, description = "No track has this artist", body = ErrorBody),
    )
)]
pub async fn artist(
    State(state): State<AppState>,
    Path(artist_index): Path<u32>,
    Query(query): Query<ArtistQuery>,
) -> Result<Json<ArtistProfileDto>, ApiError> {
    check_range("top", query.top, MAX_ARTIST_TRACKS)?;

    match state.store.artist(artist_index, query.top).await? {
        Some(profile) => Ok(Json(profile.into())),
        None => Err(ApiError::NotFound(format!(
            "artist {artist_index} not found"
        ))),
    }
}

/// One track by its index.
#[utoipa::path(
    get,
    path = "/tracks/{track_index}",
    tag = "tracks",
    params(("track_index" = u32, Path, description = "Index of the track")),
    responses(
        (status = 200, description = "The track", body = TrackDto),
        (status = 404, description = "No such track", body = ErrorBody),
    )
)]
pub async fn track(
    State(state): State<AppState>,
    Path(track_index): Path<u32>,
) -> Result<Json<TrackDto>, ApiError> {
    match state.store.track(track_index).await? {
        Some(track) => Ok(Json(track.into())),
        None => Err(ApiError::NotFound(format!("track {track_index} not found"))),
    }
}

/// A listener's plays page by page, newest first, with track names.
#[utoipa::path(
    get,
    path = "/users/{user_index}/history",
    tag = "users",
    params(
        ("user_index" = u32, Path, description = "Index of the user"),
        HistoryQuery,
    ),
    responses(
        (status = 200, description = "One page of plays", body = HistoryResponse),
        (status = 400, description = "`limit` is out of range", body = ErrorBody),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn user_history(
    State(state): State<AppState>,
    Path(user_index): Path<u32>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, ApiError> {
    check_range("limit", query.limit, MAX_HISTORY_PAGE)?;

    let page = state
        .store
        .history_page(user_index, query.limit, query.offset)
        .await?;
    Ok(Json(HistoryResponse::new(
        user_index,
        query.limit,
        query.offset,
        page,
    )))
}

/// A playlist for a listener. Each track is chosen as if the ones before it had
/// already been played, with no repeats and no more than three tracks of one
/// artist. Listeners without plays start from the cold start tracks.
#[utoipa::path(
    get,
    path = "/users/{user_index}/radio",
    tag = "recommendations",
    params(
        ("user_index" = u32, Path, description = "Index of the user"),
        RadioQuery,
    ),
    responses(
        (status = 200, description = "The playlist in playing order", body = RadioResponse),
        (status = 400, description = "`length` is out of range", body = ErrorBody),
        (status = 404, description = "No such user", body = ErrorBody),
    )
)]
pub async fn radio(
    State(state): State<AppState>,
    Path(user_index): Path<u32>,
    Query(query): Query<RadioQuery>,
) -> Result<Json<RadioResponse>, ApiError> {
    check_range("length", query.length, MAX_RADIO_LENGTH)?;

    let Some(user) = state.store.user(user_index).await? else {
        return Err(ApiError::NotFound(format!("user {user_index} not found")));
    };
    let history = state.store.history(user_index).await?;
    let history_len = history.len();

    // one ranking per track: CPU work, keep it off the async threads
    let recommender = state.recommender.clone();
    let length = query.length;
    let playlist = tokio::task::spawn_blocking(move || {
        let recommender = recommender
            .read()
            .expect("the recommender lock is never poisoned");
        recommender.radio(history, user.country.as_deref(), length)
    })
    .await?;

    Ok(Json(RadioResponse {
        user: user_index,
        history_len,
        items: describe(&state, playlist).await?,
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::api::test_support::{app, get_json, send};

    #[tokio::test]
    async fn overview_counts_the_window() {
        let (status, body) = get_json("/stats/overview?days=7&top=2").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["days"], 7);
        assert_eq!(body["plays"], 5);
        assert_eq!(body["listeners"], 2);
        assert_eq!(body["top_tracks"].as_array().unwrap().len(), 2);
        assert_eq!(body["by_hour"].as_array().unwrap().len(), 24);
        assert_eq!(body["by_weekday"].as_array().unwrap().len(), 7);
    }

    #[tokio::test]
    async fn overview_rejects_a_window_out_of_range() {
        let (status, body) = get_json("/stats/overview?days=0").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("days"));
        let (status, _) = get_json("/stats/overview?days=366").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn artist_search_needs_two_characters() {
        let (status, _) = get_json("/artists?q=a").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, body) = get_json("/artists?q=zz").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn artist_page_has_top_tracks() {
        let (status, body) = get_json("/artists/0?top=2").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["plays"], 5);
        assert_eq!(body["tracks"], 4);
        let top = body["top_tracks"].as_array().unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0]["plays"], 2);

        let (status, body) = get_json("/artists/9").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("artist 9"));
    }

    #[tokio::test]
    async fn track_is_found_by_index() {
        let (status, body) = get_json("/tracks/12").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["track_index"], 12);
        assert_eq!(body["artist_index"], 0);

        let (status, _) = get_json("/tracks/999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn history_is_paged() {
        let (status, body) = get_json("/users/2/history?limit=2&offset=1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 3);
        assert_eq!(
            (body["limit"].clone(), body["offset"].clone()),
            (2.into(), 1.into())
        );
        let items = body["items"].as_array().unwrap();
        // newest first: 12, 11, 10, so the page after the first one is 11, 10
        assert_eq!(items[0]["track_index"], 11);
        assert_eq!(items[1]["track_index"], 10);

        let (status, _) = get_json("/users/99/history").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = get_json("/users/2/history?limit=0").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn radio_never_repeats_and_skips_heard_tracks() {
        let app = app().await;
        let (status, body) = send(&app, "GET", "/users/1/radio?length=5", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["history_len"], 2);

        let tracks: Vec<u64> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["track_index"].as_u64().unwrap())
            .collect();
        // only 12 and 13 are left unheard, and the popular fallback adds nothing new
        assert!(!tracks.is_empty() && tracks.len() <= 2);
        assert!(tracks.iter().all(|track| *track == 12 || *track == 13));
        let unique: std::collections::HashSet<u64> = tracks.iter().copied().collect();
        assert_eq!(unique.len(), tracks.len());
    }

    #[tokio::test]
    async fn radio_checks_the_length_and_the_user() {
        let (status, _) = get_json("/users/1/radio?length=0").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = get_json("/users/1/radio?length=51").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = get_json("/users/99/radio").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
