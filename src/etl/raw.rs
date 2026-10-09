use chrono::NaiveDateTime;
use serde::Deserialize;

use crate::domain::{ListenEvent, Track, User};

/// One CSV row exactly as it appears in the file.
#[derive(Debug, Deserialize)]
pub struct RawListenEvent {
    pub user_index: u32,
    pub artist_index: u32,
    pub track_index: u32,
    pub timestamp: NaiveDateTime,
    pub artist_name: Option<String>,
    pub track_name: Option<String>,
    pub gender: Option<String>,
    pub age: Option<f32>,
    pub country: Option<String>,
}

impl From<&RawListenEvent> for ListenEvent {
    fn from(raw: &RawListenEvent) -> Self {
        ListenEvent {
            user_index: raw.user_index,
            track_index: raw.track_index,
            timestamp: raw.timestamp,
        }
    }
}

impl From<&RawListenEvent> for User {
    fn from(raw: &RawListenEvent) -> Self {
        User {
            user_index: raw.user_index,
            gender: raw.gender.clone(),
            age: raw.age,
            country: raw.country.clone(),
        }
    }
}

impl From<&RawListenEvent> for Track {
    fn from(raw: &RawListenEvent) -> Self {
        Track {
            track_index: raw.track_index,
            artist_index: raw.artist_index,
            track_name: raw.track_name.clone(),
            artist_name: raw.artist_name.clone(),
        }
    }
}