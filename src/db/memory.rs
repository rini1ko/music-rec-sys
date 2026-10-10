use std::collections::BTreeMap;

use crate::db::error::DbError;
use crate::db::repository::{Counts, Repository};
use crate::domain::{ListenEvent, Session, Track, User};
use chrono::Duration;

use crate::models::trends::trending;

/// Repository that keeps everything in memory. BTreeMap is used instead of
/// HashMap so that iteration order is the same on every run.
#[derive(Debug, Default)]
pub struct MemoryRepository {
    users: BTreeMap<u32, User>,
    tracks: BTreeMap<u32, Track>,
    events_by_user: BTreeMap<u32, Vec<ListenEvent>>,
    sessions: Vec<Session>,
}

impl MemoryRepository {
    fn ensure_user(&self, user_index: u32) -> Result<(), DbError> {
        if self.users.contains_key(&user_index) {
            Ok(())
        } else {
            Err(DbError::UserNotFound(user_index))
        }
    }

    fn ensure_track(&self, track_index: u32) -> Result<(), DbError> {
        if self.tracks.contains_key(&track_index) {
            Ok(())
        } else {
            Err(DbError::TrackNotFound(track_index))
        }
    }
}

impl Repository for MemoryRepository {
    fn add_users(&mut self, users: Vec<User>) -> Result<(), DbError> {
        for user in users {
            self.users.entry(user.user_index).or_insert(user);
        }
        Ok(())
    }

    fn add_tracks(&mut self, tracks: Vec<Track>) -> Result<(), DbError> {
        for track in tracks {
            self.tracks.entry(track.track_index).or_insert(track);
        }
        Ok(())
    }

    fn add_events(&mut self, events: Vec<ListenEvent>) -> Result<(), DbError> {
        // the whole batch is validated first, so a failure leaves the store untouched
        for event in &events {
            self.ensure_user(event.user_index)?;
            self.ensure_track(event.track_index)?;
        }
        for event in events {
            self.events_by_user
                .entry(event.user_index)
                .or_default()
                .push(event);
        }
        Ok(())
    }

    fn add_sessions(&mut self, sessions: Vec<Session>) -> Result<(), DbError> {
        for session in &sessions {
            self.ensure_user(session.user_index)?;
            for &track_index in &session.tracks {
                self.ensure_track(track_index)?;
            }
        }
        self.sessions.extend(sessions);
        Ok(())
    }

    fn all_events(&self) -> Result<Vec<ListenEvent>, DbError> {
        Ok(self.events_by_user.values().flatten().cloned().collect())
    }

    fn all_sessions(&self) -> Result<Vec<Session>, DbError> {
        Ok(self.sessions.clone())
    }

    fn user_history(&self, user_index: u32) -> Result<Vec<u32>, DbError> {
        self.ensure_user(user_index)?;

        let Some(events) = self.events_by_user.get(&user_index) else {
            return Ok(Vec::new());
        };

        let mut ordered: Vec<&ListenEvent> = events.iter().collect();
        ordered.sort_by_key(|event| event.timestamp);
        Ok(ordered.into_iter().map(|event| event.track_index).collect())
    }

    fn get_user(&self, user_index: u32) -> Result<Option<User>, DbError> {
        Ok(self.users.get(&user_index).cloned())
    }

    fn get_track(&self, track_index: u32) -> Result<Option<Track>, DbError> {
        Ok(self.tracks.get(&track_index).cloned())
    }

    fn counts(&self) -> Result<Counts, DbError> {
        Ok(Counts {
            users: self.users.len(),
            tracks: self.tracks.len(),
            events: self.events_by_user.values().map(Vec::len).sum(),
            sessions: self.sessions.len(),
        })
    }

    fn trending(&self, window: Duration, limit: usize) -> Result<Vec<(u32, usize)>, DbError> {
        Ok(trending(&self.all_events()?, window, limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, session_of, track, user};

    fn repo_with(users: &[u32], tracks: &[u32]) -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.add_users(users.iter().map(|&u| user(u)).collect())
            .unwrap();
        repo.add_tracks(tracks.iter().map(|&t| track(t)).collect())
            .unwrap();
        repo
    }

    #[test]
    fn events_need_a_known_user() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(2, 10, 0)]);
        assert_eq!(result, Err(DbError::UserNotFound(2)));
    }

    #[test]
    fn events_need_a_known_track() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(1, 99, 0)]);
        assert_eq!(result, Err(DbError::TrackNotFound(99)));
    }

    #[test]
    fn failed_batch_stores_nothing() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(1, 10, 0), event_of(1, 99, 1)]);

        assert!(result.is_err());
        assert_eq!(repo.counts().unwrap().events, 0);
    }

    #[test]
    fn sessions_need_known_tracks() {
        let mut repo = repo_with(&[0], &[10]);
        let result = repo.add_sessions(vec![session_of(0, &[10, 99])]);

        assert_eq!(result, Err(DbError::TrackNotFound(99)));
        assert_eq!(repo.counts().unwrap().sessions, 0);
    }

    #[test]
    fn first_version_of_a_user_wins() {
        let mut repo = MemoryRepository::default();
        let mut first = user(1);
        first.country = Some("UK".to_string());
        let mut second = user(1);
        second.country = Some("DE".to_string());

        repo.add_users(vec![first]).unwrap();
        repo.add_users(vec![second]).unwrap();

        let stored = repo.get_user(1).unwrap().unwrap();
        assert_eq!(stored.country, Some("UK".to_string()));
    }

    #[test]
    fn history_is_ordered_by_time() {
        let mut repo = repo_with(&[1], &[10, 11, 12]);
        repo.add_events(vec![
            event_of(1, 10, 5),
            event_of(1, 11, 0),
            event_of(1, 12, 3),
        ])
        .unwrap();

        assert_eq!(repo.user_history(1), Ok(vec![11, 12, 10]));
    }

    #[test]
    fn history_of_unknown_user_is_an_error() {
        let repo = MemoryRepository::default();
        assert_eq!(repo.user_history(7), Err(DbError::UserNotFound(7)));
    }

    #[test]
    fn user_without_events_has_empty_history() {
        let repo = repo_with(&[1], &[]);
        assert_eq!(repo.user_history(1), Ok(Vec::new()));
    }

    #[test]
    fn counts_reflect_stored_data() {
        let mut repo = repo_with(&[1, 2], &[10, 11]);
        repo.add_events(vec![
            event_of(1, 10, 0),
            event_of(2, 11, 0),
            event_of(2, 10, 1),
        ])
        .unwrap();
        repo.add_sessions(vec![session_of(1, &[10, 11])]).unwrap();

        assert_eq!(
            repo.counts().unwrap(),
            Counts {
                users: 2,
                tracks: 2,
                events: 3,
                sessions: 1,
            }
        );
    }

    #[test]
    fn unknown_track_is_none() {
        let repo = MemoryRepository::default();
        assert_eq!(repo.get_track(5), Ok(None));
    }
}