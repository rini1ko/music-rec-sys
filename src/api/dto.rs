use serde::{Deserialize, Serialize};

use crate::domain::Track;
use crate::ranker::Recommendation;

fn default_top() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct RecommendQuery {
    pub user: u32,
    #[serde(default = "default_top")]
    pub top: usize,
}

#[derive(Debug, Serialize)]
pub struct ReasonDto {
    pub source: &'static str,
    pub text: &'static str,
    pub contribution: f64,
}

#[derive(Debug, Serialize)]
pub struct RecommendationDto {
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub score: f64,
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

#[derive(Debug, Serialize)]
pub struct RecommendationsResponse {
    pub user: u32,
    pub history_len: usize,
    pub items: Vec<RecommendationDto>,
}