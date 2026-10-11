use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::{
    ArtistProfile, ArtistSummary, HistoryPage, StatsOverview, Track, TrackPlays, UserSummary,
};
use crate::engine::Weights;
use crate::ranker::Recommendation;

fn default_top() -> usize {
    10
}

fn default_users_limit() -> usize {
    50
}

fn default_search_limit() -> usize {
    20
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthDto {
    pub status: &'static str,
    pub users: usize,
    pub tracks: usize,
    pub events: usize,
    pub sessions: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

/// Default weight of each model. A request to `/recommendations` can override
/// any of them for itself.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct WeightsDto {
    pub trends: f64,
    pub markov: f64,
    pub co_listening: f64,
    pub cf: f64,
}

impl From<Weights> for WeightsDto {
    fn from(weights: Weights) -> Self {
        Self {
            trends: weights.trends,
            markov: weights.markov,
            co_listening: weights.co_listening,
            cf: weights.cf,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecommendQuery {
    /// Index of the user to recommend for.
    pub user: u32,
    /// How many tracks to return, from 1 to 100. Default is 10.
    #[serde(default = "default_top")]
    pub top: usize,
    /// Weight of the trends model for this request only, from 0 to 10.
    pub trends: Option<f64>,
    /// Weight of the next-track (Markov) model for this request only.
    pub markov: Option<f64>,
    /// Weight of the co-listening model for this request only.
    pub co_listening: Option<f64>,
    /// Weight of the collaborative filtering model for this request only.
    pub cf: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReasonDto {
    /// Machine name of the source: trends, markov, co_listening, cf, artist,
    /// country or popular.
    pub source: &'static str,
    /// Human-readable explanation.
    pub text: &'static str,
    /// How much this model added to the final score.
    pub contribution: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecommendationDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// `None` only when the track is missing from the catalog.
    pub artist_index: Option<u32>,
    pub score: f64,
    /// Strongest reason first.
    pub reasons: Vec<ReasonDto>,
}

impl RecommendationDto {
    pub fn new(recommendation: Recommendation, track: Option<Track>) -> Self {
        let (title, artist, artist_index) = match track {
            Some(track) => (
                track.track_name,
                track.artist_name,
                Some(track.artist_index),
            ),
            None => (None, None, None),
        };
        Self {
            track_index: recommendation.track_index,
            title,
            artist,
            artist_index,
            score: recommendation.score,
            reasons: recommendation
                .reasons
                .iter()
                .map(|reason| ReasonDto {
                    source: reason.source.name(),
                    text: reason.source.reason(),
                    contribution: reason.contribution,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecommendationsResponse {
    pub user: u32,
    pub history_len: usize,
    pub items: Vec<RecommendationDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewEvent {
    pub user: u32,
    pub track: u32,
    /// The data lives in 2009, so "now" means right after the newest stored event.
    pub timestamp: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EventDto {
    pub user: u32,
    pub track: u32,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrackDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// Opens the artist page: `/artists/{artist_index}`.
    pub artist_index: u32,
}

impl From<Track> for TrackDto {
    fn from(track: Track) -> Self {
        Self {
            track_index: track.track_index,
            title: track.track_name,
            artist: track.artist_name,
            artist_index: track.artist_index,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserDto {
    pub user_index: u32,
    pub gender: Option<String>,
    pub age: Option<f32>,
    pub country: Option<String>,
    pub history_len: usize,
    /// Newest first.
    pub recent: Vec<TrackDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UsersQuery {
    /// How many users to return, from 1 to 200. Default is 50.
    #[serde(default = "default_users_limit")]
    pub limit: usize,
    /// How many users to skip. Default is 0.
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserSummaryDto {
    pub user_index: u32,
    pub gender: Option<String>,
    pub age: Option<f32>,
    pub country: Option<String>,
    /// How many tracks the user listened to.
    pub plays: usize,
}

impl From<UserSummary> for UserSummaryDto {
    fn from(summary: UserSummary) -> Self {
        Self {
            user_index: summary.user.user_index,
            gender: summary.user.gender,
            age: summary.user.age,
            country: summary.user.country,
            plays: summary.plays,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UsersResponse {
    /// All users in the database, not only this page.
    pub total: usize,
    /// Most active first.
    pub items: Vec<UserSummaryDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TrackSearchQuery {
    /// Part of a track or artist name, at least 2 characters.
    pub q: String,
    /// How many tracks to return, from 1 to 50. Default is 20.
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SimilarQuery {
    /// How many tracks to return, from 1 to 100. Default is 10.
    #[serde(default = "default_top")]
    pub top: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewUser {
    pub country: Option<String>,
    /// From 1 to 120.
    pub age: Option<f32>,
    pub gender: Option<String>,
}

fn default_days() -> u32 {
    30
}

fn default_history_limit() -> usize {
    50
}

fn default_radio_length() -> usize {
    20
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OverviewQuery {
    /// Length of the window in days, from 1 to 365. Default is 30. The window
    /// ends at the newest stored event, not today: the data is from 2009.
    #[serde(default = "default_days")]
    pub days: u32,
    /// Length of the top lists, from 1 to 50. Default is 10.
    #[serde(default = "default_top")]
    pub top: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrackPlaysDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub artist_index: u32,
    pub plays: usize,
}

impl From<TrackPlays> for TrackPlaysDto {
    fn from(item: TrackPlays) -> Self {
        Self {
            track_index: item.track.track_index,
            title: item.track.track_name,
            artist: item.track.artist_name,
            artist_index: item.track.artist_index,
            plays: item.plays,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArtistDto {
    pub artist_index: u32,
    pub name: Option<String>,
    /// Tracks of the artist: all of them in a search, played ones in the statistics.
    pub tracks: usize,
    pub plays: usize,
    /// Different users who played the artist.
    pub listeners: usize,
}

impl From<ArtistSummary> for ArtistDto {
    fn from(item: ArtistSummary) -> Self {
        Self {
            artist_index: item.artist_index,
            name: item.artist_name,
            tracks: item.tracks,
            plays: item.plays,
            listeners: item.listeners,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DayPlaysDto {
    pub day: NaiveDate,
    pub plays: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CountryPlaysDto {
    /// `null` stands for listeners without a country.
    pub country: Option<String>,
    pub listeners: usize,
    pub plays: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OverviewDto {
    pub days: u32,
    /// Start of the window, `null` when there are no events.
    pub from: Option<NaiveDateTime>,
    /// The newest event, `null` when there are no events.
    pub to: Option<NaiveDateTime>,
    pub plays: usize,
    pub listeners: usize,
    /// Different tracks played in the window.
    pub tracks: usize,
    /// Different artists played in the window.
    pub artists: usize,
    pub top_tracks: Vec<TrackPlaysDto>,
    pub top_artists: Vec<ArtistDto>,
    /// Days without plays are missing.
    pub by_day: Vec<DayPlaysDto>,
    /// 24 numbers, index is the hour.
    pub by_hour: Vec<usize>,
    /// 7 numbers, index 0 is Sunday.
    pub by_weekday: Vec<usize>,
    /// Most plays first.
    pub countries: Vec<CountryPlaysDto>,
}

impl OverviewDto {
    pub fn new(days: u32, overview: StatsOverview) -> Self {
        Self {
            days,
            from: overview.window.map(|(from, _)| from),
            to: overview.window.map(|(_, to)| to),
            plays: overview.plays,
            listeners: overview.listeners,
            tracks: overview.tracks,
            artists: overview.artists,
            top_tracks: overview.top_tracks.into_iter().map(Into::into).collect(),
            top_artists: overview.top_artists.into_iter().map(Into::into).collect(),
            by_day: overview
                .by_day
                .into_iter()
                .map(|item| DayPlaysDto {
                    day: item.day,
                    plays: item.plays,
                })
                .collect(),
            by_hour: overview.by_hour.to_vec(),
            by_weekday: overview.by_weekday.to_vec(),
            countries: overview
                .countries
                .into_iter()
                .map(|item| CountryPlaysDto {
                    country: item.country,
                    listeners: item.listeners,
                    plays: item.plays,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ArtistSearchQuery {
    /// Part of the artist name, at least 2 characters.
    pub q: String,
    /// How many artists to return, from 1 to 50. Default is 20.
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ArtistQuery {
    /// How many top tracks to return, from 1 to 100. Default is 10.
    #[serde(default = "default_top")]
    pub top: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArtistProfileDto {
    pub artist_index: u32,
    pub name: Option<String>,
    pub tracks: usize,
    pub plays: usize,
    pub listeners: usize,
    /// Most played first.
    pub top_tracks: Vec<TrackPlaysDto>,
}

impl From<ArtistProfile> for ArtistProfileDto {
    fn from(profile: ArtistProfile) -> Self {
        let summary = profile.summary;
        Self {
            artist_index: summary.artist_index,
            name: summary.artist_name,
            tracks: summary.tracks,
            plays: summary.plays,
            listeners: summary.listeners,
            top_tracks: profile.top_tracks.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    /// How many plays to return, from 1 to 200. Default is 50.
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    /// How many of the newest plays to skip.
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HistoryEntryDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub artist_index: u32,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HistoryResponse {
    pub user: u32,
    /// All plays of the user, not only this page.
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    /// Newest first.
    pub items: Vec<HistoryEntryDto>,
}

impl HistoryResponse {
    pub fn new(user: u32, limit: usize, offset: usize, page: HistoryPage) -> Self {
        Self {
            user,
            total: page.total,
            limit,
            offset,
            items: page
                .items
                .into_iter()
                .map(|entry| HistoryEntryDto {
                    track_index: entry.track.track_index,
                    title: entry.track.track_name,
                    artist: entry.track.artist_name,
                    artist_index: entry.track.artist_index,
                    timestamp: entry.timestamp,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RadioQuery {
    /// How many tracks the playlist should have, from 1 to 50. Default is 20.
    #[serde(default = "default_radio_length")]
    pub length: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RadioResponse {
    pub user: u32,
    pub history_len: usize,
    /// In playing order. Each track was chosen as if the ones before it had been played.
    pub items: Vec<RecommendationDto>,
}
