use chrono::{NaiveDate, NaiveDateTime};

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

/// A user with the number of tracks they listened to.
#[derive(Debug, Clone, PartialEq)]
pub struct UserSummary {
    pub user: User,
    pub plays: usize,
}

/// One page of users plus the total number of users.
#[derive(Debug, Clone, PartialEq)]
pub struct UsersPage {
    pub total: usize,
    pub items: Vec<UserSummary>,
}

/// A track with how many times it was played.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPlays {
    pub track: Track,
    pub plays: usize,
}

/// An artist with play statistics, either for all data or for a time window.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistSummary {
    pub artist_index: u32,
    pub artist_name: Option<String>,
    /// Distinct tracks: all tracks of the artist in a search, played ones in a window.
    pub tracks: usize,
    pub plays: usize,
    pub listeners: usize,
}

/// An artist with their most played tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistProfile {
    pub summary: ArtistSummary,
    pub top_tracks: Vec<TrackPlays>,
}

/// Plays of listeners from one country. `None` groups listeners without a country.
#[derive(Debug, Clone, PartialEq)]
pub struct CountryPlays {
    pub country: Option<String>,
    pub listeners: usize,
    pub plays: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayPlays {
    pub day: NaiveDate,
    pub plays: usize,
}

/// What happened in the last days of the data.
#[derive(Debug, Clone, PartialEq)]
pub struct StatsOverview {
    /// First and last moment of the window. `None` when there are no events at all.
    pub window: Option<(NaiveDateTime, NaiveDateTime)>,
    pub plays: usize,
    pub listeners: usize,
    pub tracks: usize,
    pub artists: usize,
    pub top_tracks: Vec<TrackPlays>,
    pub top_artists: Vec<ArtistSummary>,
    /// Every day of the window, days without plays have 0.
    pub by_day: Vec<DayPlays>,
    /// Index is the hour of the day, 0 to 23.
    pub by_hour: [usize; 24],
    /// Index is the day of the week, 0 is Sunday.
    pub by_weekday: [usize; 7],
    pub countries: Vec<CountryPlays>,
}

/// One play in a listener's history.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub track: Track,
    pub timestamp: NaiveDateTime,
}

/// One page of a listener's history, newest first, plus the total number of plays.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPage {
    pub total: usize,
    pub items: Vec<HistoryEntry>,
}

/// What a listener said about a recommended track (R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedbackKind {
    Like,
    Skip,
}

impl FeedbackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FeedbackKind::Like => "like",
            FeedbackKind::Skip => "skip",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "like" => Some(FeedbackKind::Like),
            "skip" => Some(FeedbackKind::Skip),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feedback {
    pub user_index: u32,
    pub track_index: u32,
    pub kind: FeedbackKind,
    /// Name of the model credited with the track, `None` if no model proposed it.
    pub source: Option<String>,
    pub created_at: NaiveDateTime,
}

/// A feedback with the track it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackEntry {
    pub feedback: Feedback,
    pub track: Track,
}

/// Likes and skips of one listener for the tracks of one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFeedback {
    pub source: String,
    pub likes: u32,
    pub skips: u32,
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

    /// A track with a title and an artist, for tests that read names back.
    pub fn named_track(track_index: u32, artist_index: u32, title: &str, artist: &str) -> Track {
        Track {
            track_index,
            artist_index,
            track_name: Some(title.to_string()),
            artist_name: Some(artist.to_string()),
        }
    }

    pub fn user_from(user_index: u32, country: &str) -> User {
        User {
            country: Some(country.to_string()),
            ..user(user_index)
        }
    }
}
