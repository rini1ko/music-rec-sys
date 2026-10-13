//! `etl --append` (E6): adds a CSV to a database that already has data.
//! Users and tracks that exist are kept, plays that exist (same user, track
//! and time) are skipped, and the sessions are cut again from all plays, so a
//! session that goes on across the two files stays one session.

use std::collections::HashSet;

use chrono::{Duration, NaiveDateTime};

use crate::db::error::DbError;
use crate::db::repository::Repository;
use crate::etl::sessionizer::sessionize_all;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppendReport {
    pub new_users: usize,
    pub new_tracks: usize,
    pub new_events: usize,
    /// Plays of the file that were stored already.
    pub duplicate_events: usize,
    /// All sessions after cutting them again.
    pub sessions: usize,
}

/// Copies into `repo` what `incoming` (usually the CSV loaded into memory) has
/// and `repo` lacks.
///
/// # Errors
///
/// Database errors of `repo`; a failed batch stores nothing.
pub fn append(
    repo: &mut (impl Repository + ?Sized),
    incoming: &(impl Repository + ?Sized),
    session_gap: Duration,
) -> Result<AppendReport, DbError> {
    let known_users: HashSet<u32> = repo
        .all_users()?
        .iter()
        .map(|user| user.user_index)
        .collect();
    let users: Vec<_> = incoming
        .all_users()?
        .into_iter()
        .filter(|user| !known_users.contains(&user.user_index))
        .collect();

    let known_tracks: HashSet<u32> = repo
        .all_tracks()?
        .iter()
        .map(|track| track.track_index)
        .collect();
    let tracks: Vec<_> = incoming
        .all_tracks()?
        .into_iter()
        .filter(|track| !known_tracks.contains(&track.track_index))
        .collect();

    let known_events: HashSet<(u32, u32, NaiveDateTime)> = repo
        .all_events()?
        .iter()
        .map(|event| (event.user_index, event.track_index, event.timestamp))
        .collect();
    let mut seen = known_events;
    let mut duplicate_events = 0;
    let mut events = Vec::new();
    for event in incoming.all_events()? {
        // `insert` is false for a play stored before or met twice in the file
        if seen.insert((event.user_index, event.track_index, event.timestamp)) {
            events.push(event);
        } else {
            duplicate_events += 1;
        }
    }

    let mut report = AppendReport {
        new_users: users.len(),
        new_tracks: tracks.len(),
        new_events: events.len(),
        duplicate_events,
        sessions: 0,
    };
    repo.add_users(users)?;
    repo.add_tracks(tracks)?;
    repo.add_events(events)?;

    let sessions = sessionize_all(repo.all_events()?, session_gap);
    report.sessions = sessions.len();
    repo.replace_sessions(sessions)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory::MemoryRepository;
    use crate::domain::test_utils::{event_of, track, user};

    fn repo_with(
        users: &[u32],
        tracks: &[u32],
        events: Vec<crate::domain::ListenEvent>,
    ) -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.add_users(users.iter().map(|&index| user(index)).collect())
            .unwrap();
        repo.add_tracks(tracks.iter().map(|&index| track(index)).collect())
            .unwrap();
        repo.add_events(events).unwrap();
        repo
    }

    #[test]
    fn only_new_data_is_added_and_sessions_are_cut_again() {
        let mut stored = repo_with(&[1], &[10, 11], vec![event_of(1, 10, 0)]);
        // the file repeats the old play and continues the same session 5 minutes later
        let file = repo_with(
            &[1, 2],
            &[10, 11, 12],
            vec![event_of(1, 10, 0), event_of(1, 11, 5), event_of(2, 12, 0)],
        );

        let report = append(&mut stored, &file, Duration::minutes(30)).unwrap();
        assert_eq!(
            report,
            AppendReport {
                new_users: 1,
                new_tracks: 1,
                new_events: 2,
                duplicate_events: 1,
                sessions: 2,
            }
        );
        assert_eq!(stored.all_events().unwrap().len(), 3);
        // user 1's two plays are one session across the two loads
        let sessions = stored.all_sessions().unwrap();
        assert!(
            sessions
                .iter()
                .any(|session| session.tracks == vec![10, 11])
        );
    }

    #[test]
    fn appending_the_same_file_twice_changes_nothing() {
        let file = repo_with(&[1], &[10], vec![event_of(1, 10, 0)]);
        let mut stored = MemoryRepository::default();
        append(&mut stored, &file, Duration::minutes(30)).unwrap();
        let again = append(&mut stored, &file, Duration::minutes(30)).unwrap();
        assert_eq!((again.new_events, again.duplicate_events), (0, 1));
        assert_eq!(stored.all_sessions().unwrap().len(), 1);
    }
}
