use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::{Track, UserSummary};
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
    pub score: f64,
    /// Strongest reason first.
    pub reasons: Vec<ReasonDto>,
}

impl RecommendationDto {
    pub fn new(recommendation: Recommendation, track: Option<Track>) -> Self {
        let (title, artist) = match track {
            Some(track) => (track.track_name, track.artist_name),
            None => (None, None),
        };
        Self {
            track_index: recommendation.track_index,
            title,
            artist,
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
}

impl From<Track> for TrackDto {
    fn from(track: Track) -> Self {
        Self {
            track_index: track.track_index,
            title: track.track_name,
            artist: track.artist_name,
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
