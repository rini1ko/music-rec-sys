use chrono::Duration;

use crate::domain::{ListenEvent, Session};

/// Takes events of ONE user and splits them into sessions.
/// A new session starts when the pause between two events is longer than `gap`.
pub fn sessionize(mut events: Vec<ListenEvent>, gap: Duration) -> Vec<Session> {
    events.sort_by_key(|e| e.timestamp);

    let mut sessions: Vec<Session> = Vec::new();

    for event in events {
        let start_new = sessions
            .last()
            .map_or(true, |s| event.timestamp - s.end > gap);

        if start_new {
            sessions.push(Session {
                user_index: event.user_index,
                tracks: vec![event.track_index],
                start: event.timestamp,
                end: event.timestamp,
            });
        } else if let Some(session) = sessions.last_mut() {
            session.tracks.push(event.track_index);
            session.end = event.timestamp;
        }
    }

    sessions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event;

    #[test]
    fn empty_input_gives_no_sessions() {
        assert!(sessionize(vec![], Duration::minutes(30)).is_empty());
    }

    #[test]
    fn pause_of_exactly_30_minutes_keeps_one_session() {
        let sessions = sessionize(vec![event(1, 0), event(2, 30)], Duration::minutes(30));
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn pause_of_31_minutes_splits_session() {
        let sessions = sessionize(vec![event(1, 0), event(2, 31)], Duration::minutes(30));
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn unsorted_input_is_sorted_first() {
        let sessions = sessionize(vec![event(2, 10), event(1, 0)], Duration::minutes(30));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].tracks, vec![1, 2]);
    }
}