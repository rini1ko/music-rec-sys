use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub user_index: u32,
    pub gender: Option<String>,
    pub age: Option<f32>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub track_index: u32,
    pub artist_index: u32,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListenEvent {
    pub user_index: u32,
    pub track_index: u32,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub user_index: u32,
    pub tracks: Vec<u32>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

/// Helpers shared by tests in other modules.
#[cfg(test)]
pub mod test_utils {
    use super::*;
    use chrono::{Duration, NaiveDate};

    pub fn event_of(user_index: u32, track_index: u32, minutes: i64) -> ListenEvent {
        let base = NaiveDate::from_ymd_opt(2009, 5, 4)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        ListenEvent {
            user_index,
            track_index,
            timestamp: base + Duration::minutes(minutes),
        }
    }

    pub fn event(track_index: u32, minutes: i64) -> ListenEvent {
        event_of(0, track_index, minutes)
    }

    pub fn session_of(user_index: u32, tracks: &[u32]) -> Session {
        let t = event(0, 0).timestamp;
        Session {
            user_index,
            tracks: tracks.to_vec(),
            start: t,
            end: t,
        }
    }

    pub fn session(tracks: &[u32]) -> Session {
        session_of(0, tracks)
    }

    pub fn user(user_index: u32) -> User {
        User {
            user_index,
            gender: None,
            age: None,
            country: None,
        }
    }

    pub fn track(track_index: u32) -> Track {
        Track {
            track_index,
            artist_index: 0,
            track_name: None,
            artist_name: None,
        }
    }
}