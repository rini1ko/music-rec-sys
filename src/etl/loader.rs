use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use chrono::Duration;
use thiserror::Error;

use crate::db::error::DbError;
use crate::db::repository::Repository;
use crate::domain::{ListenEvent, Track, User};
use crate::etl::raw::RawListenEvent;
use crate::etl::sessionizer::sessionize;

/// Only the first few parse errors are kept, a broken file can have millions.
const MAX_REPORTED_ERRORS: usize = 5;

#[derive(Debug, Error)]
pub enum EtlError {
    #[error("failed to read input: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to read CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Db(#[from] DbError),
}

/// What one load did. Counts describe the loaded file, not the whole repository.
#[derive(Debug, Default)]
pub struct LoadReport {
    pub rows_loaded: usize,
    pub rows_skipped: usize,
    pub first_errors: Vec<String>,
    pub users: usize,
    pub tracks: usize,
    pub events: usize,
    pub sessions: usize,
}

pub fn load_from_path(
    path: impl AsRef<Path>,
    repo: &mut impl Repository,
    session_gap: Duration,
) -> Result<LoadReport, EtlError> {
    let file = File::open(path)?;
    load_from_reader(file, repo, session_gap)
}

/// Reads CSV rows, splits them into users, tracks and events, builds sessions
/// and stores everything. Rows that cannot be parsed are skipped and counted.
pub fn load_from_reader<R: Read>(
    source: R,
    repo: &mut impl Repository,
    session_gap: Duration,
) -> Result<LoadReport, EtlError> {
    let mut reader = csv::Reader::from_reader(source);

    let mut users: BTreeMap<u32, User> = BTreeMap::new();
    let mut tracks: BTreeMap<u32, Track> = BTreeMap::new();
    let mut events_by_user: BTreeMap<u32, Vec<ListenEvent>> = BTreeMap::new();
    let mut rows_loaded = 0;
    let mut rows_skipped = 0;
    let mut first_errors: Vec<String> = Vec::new();

    for result in reader.deserialize::<RawListenEvent>() {
        let raw = match result {
            Ok(raw) => raw,
            // a broken file is a real failure, a broken row is not
            Err(err) if err.is_io_error() => return Err(err.into()),
            Err(err) => {
                rows_skipped += 1;
                if first_errors.len() < MAX_REPORTED_ERRORS {
                    first_errors.push(err.to_string());
                }
                continue;
            }
        };

        rows_loaded += 1;
        users
            .entry(raw.user_index)
            .or_insert_with(|| User::from(&raw));
        tracks
            .entry(raw.track_index)
            .or_insert_with(|| Track::from(&raw));
        events_by_user
            .entry(raw.user_index)
            .or_default()
            .push(ListenEvent::from(&raw));
    }

    let mut sessions = Vec::new();
    let mut all_events: Vec<ListenEvent> = Vec::new();
    for (_, events) in events_by_user {
        sessions.extend(sessionize(events.clone(), session_gap));
        all_events.extend(events);
    }

    let report = LoadReport {
        rows_loaded,
        rows_skipped,
        first_errors,
        users: users.len(),
        tracks: tracks.len(),
        events: all_events.len(),
        sessions: sessions.len(),
    };

    // users and tracks go first, events and sessions reference them
    repo.add_users(users.into_values().collect())?;
    repo.add_tracks(tracks.into_values().collect())?;
    repo.add_events(all_events)?;
    repo.add_sessions(sessions)?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory::MemoryRepository;
    use crate::db::repository::Counts;
    use crate::domain::test_utils::user;

    const HEADER: &str =
        "user_index,artist_index,track_index,timestamp,artist_name,track_name,gender,age,country\n";

    const ROWS: &str = "0,10,100,2009-05-04T12:00:00,A,Song 1,male,30,UK\n\
0,10,101,2009-05-04T12:05:00,A,Song 2,male,30,UK\n\
0,11,102,2009-05-04T15:00:00,B,Song 3,male,30,UK\n\
1,11,102,2009-05-04T12:00:00,B,Song 3,,,\n";

    fn load(rows: &str) -> (MemoryRepository, LoadReport) {
        let mut repo = MemoryRepository::default();
        let csv = format!("{HEADER}{rows}");
        let report = load_from_reader(csv.as_bytes(), &mut repo, Duration::minutes(30)).unwrap();
        (repo, report)
    }

    #[test]
    fn loads_users_tracks_events_and_sessions() {
        let (repo, report) = load(ROWS);

        assert_eq!(report.rows_loaded, 4);
        assert_eq!(report.rows_skipped, 0);
        assert_eq!(report.users, 2);
        assert_eq!(report.tracks, 3);
        assert_eq!(report.events, 4);
        // user 0 has a 3 hour pause, so two sessions, user 1 has one
        assert_eq!(report.sessions, 3);
        assert_eq!(
            repo.counts().unwrap(),
            Counts {
                users: 2,
                tracks: 3,
                events: 4,
                sessions: 3,
            }
        );
    }

    #[test]
    fn bad_rows_are_skipped_and_reported() {
        let bad = "0,10,100,not-a-date,A,Song 1,male,30,UK\n";
        let (repo, report) = load(&format!("{ROWS}{bad}"));

        assert_eq!(report.rows_loaded, 4);
        assert_eq!(report.rows_skipped, 1);
        assert_eq!(report.first_errors.len(), 1);
        assert_eq!(repo.counts().unwrap().events, 4);
    }

    #[test]
    fn reported_errors_are_capped() {
        let bad = "x,10,100,2009-05-04T12:00:00,A,Song,,,\n".repeat(7);
        let (_, report) = load(&bad);

        assert_eq!(report.rows_skipped, 7);
        assert_eq!(report.first_errors.len(), MAX_REPORTED_ERRORS);
    }

    #[test]
    fn user_fields_come_from_the_file() {
        let (repo, _) = load(ROWS);

        let first = User {
            user_index: 0,
            gender: Some("male".to_string()),
            age: Some(30.0),
            country: Some("UK".to_string()),
        };
        assert_eq!(repo.get_user(0).unwrap(), Some(first));
        // empty cells become None
        assert_eq!(repo.get_user(1).unwrap(), Some(user(1)));
    }

    #[test]
    fn track_fields_come_from_the_file() {
        let (repo, _) = load(ROWS);

        let expected = Track {
            track_index: 100,
            artist_index: 10,
            track_name: Some("Song 1".to_string()),
            artist_name: Some("A".to_string()),
        };
        assert_eq!(repo.get_track(100).unwrap(), Some(expected));
    }

    #[test]
    fn header_only_file_loads_nothing() {
        let (repo, report) = load("");

        assert_eq!(report.rows_loaded, 0);
        assert_eq!(report.events, 0);
        assert_eq!(repo.counts().unwrap(), Counts::default());
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let mut repo = MemoryRepository::default();
        let result = load_from_path("does/not/exist.csv", &mut repo, Duration::minutes(30));
        assert!(matches!(result, Err(EtlError::Io(_))));
    }
}