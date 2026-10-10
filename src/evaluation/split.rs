use std::collections::BTreeMap;

use crate::domain::ListenEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitMode {
    /// The newest part of EACH user's own history is held out. Almost every
    /// user gets evaluated, but models can learn from other users' "future".
    #[default]
    PerUser,
    /// One cutoff moment for everybody. Stricter, but only users who were
    /// active after the cutoff can be evaluated.
    Global,
}

pub fn split(events: &[ListenEvent], mode: SplitMode, test_fraction: f64) -> Split {
    match mode {
        SplitMode::PerUser => split_per_user(events, test_fraction),
        SplitMode::Global => split_by_time(events, test_fraction),
    }
}

pub struct Split {
    pub train: Vec<ListenEvent>,
    pub test: Vec<ListenEvent>,
}

/// Splits events by time: everything before the cutoff is for training,
/// everything from the cutoff on is for testing. A random split would let the
/// models "see the future", which a real recommender never can.
///
/// `test_fraction` must be inside (0, 1), the caller validates it.
pub fn split_by_time(events: &[ListenEvent], test_fraction: f64) -> Split {
    if events.is_empty() {
        return Split {
            train: Vec::new(),
            test: Vec::new(),
        };
    }

    let mut timestamps: Vec<_> = events.iter().map(|e| e.timestamp).collect();
    timestamps.sort_unstable();

    let cutoff_index = ((timestamps.len() as f64) * (1.0 - test_fraction)).floor() as usize;
    let cutoff = timestamps[cutoff_index.min(timestamps.len() - 1)];

    let (train, test) = events.iter().cloned().partition(|e| e.timestamp < cutoff);
    Split { train, test }
}

/// Holds out the newest `test_fraction` of every user's own events.
/// A user with a single event stays in training completely.
pub fn split_per_user(events: &[ListenEvent], test_fraction: f64) -> Split {
    let mut by_user: BTreeMap<u32, Vec<&ListenEvent>> = BTreeMap::new();
    for event in events {
        by_user.entry(event.user_index).or_default().push(event);
    }

    let mut train = Vec::new();
    let mut test = Vec::new();
    for (_, mut user_events) in by_user {
        user_events.sort_by_key(|e| e.timestamp);
        let count = user_events.len();
        if count < 2 {
            train.extend(user_events.into_iter().cloned());
            continue;
        }

        let test_count = ((count as f64 * test_fraction).round() as usize).clamp(1, count - 1);
        let cut = count - test_count;
        train.extend(user_events[..cut].iter().map(|e| (*e).clone()));
        test.extend(user_events[cut..].iter().map(|e| (*e).clone()));
    }
    Split { train, test }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event;

    #[test]
    fn newest_events_go_to_test() {
        let events: Vec<_> = (0..10).map(|i| event(i, i64::from(i))).collect();
        let split = split_by_time(&events, 0.2);

        assert_eq!(split.train.len(), 8);
        assert_eq!(split.test.len(), 2);
        let latest_train = split.train.iter().map(|e| e.timestamp).max().unwrap();
        let earliest_test = split.test.iter().map(|e| e.timestamp).min().unwrap();
        assert!(latest_train < earliest_test);
    }

    #[test]
    fn input_order_does_not_matter() {
        let events = vec![event(3, 30), event(1, 10), event(2, 20), event(4, 40)];
        let split = split_by_time(&events, 0.5);

        let mut test_tracks: Vec<u32> = split.test.iter().map(|e| e.track_index).collect();
        test_tracks.sort_unstable();
        assert_eq!(test_tracks, vec![3, 4]);
    }

    #[test]
    fn events_with_the_same_timestamp_stay_together() {
        let events = vec![event(1, 0), event(2, 5), event(3, 5), event(4, 5)];
        let split = split_by_time(&events, 0.5);

        assert_eq!(split.train.len(), 1);
        assert_eq!(split.test.len(), 3);
    }

    #[test]
    fn empty_input_gives_empty_split() {
        let split = split_by_time(&[], 0.2);
        assert!(split.train.is_empty() && split.test.is_empty());
    }

    #[test]
    fn per_user_split_holds_out_each_users_newest_events() {
        use crate::domain::test_utils::event_of;

        let events = vec![
            event_of(0, 1, 0),
            event_of(0, 2, 1),
            event_of(0, 3, 2),
            event_of(0, 4, 3),
            // another user who listened much later
            event_of(1, 5, 1000),
            event_of(1, 6, 1001),
        ];
        let split = split_per_user(&events, 0.5);

        let mut test_tracks: Vec<u32> = split.test.iter().map(|e| e.track_index).collect();
        test_tracks.sort_unstable();
        assert_eq!(test_tracks, vec![3, 4, 6]);
        assert_eq!(split.train.len(), 3);
    }

    #[test]
    fn per_user_split_keeps_at_least_one_event_in_each_part() {
        use crate::domain::test_utils::event_of;

        let events = vec![event_of(0, 1, 0), event_of(0, 2, 1), event_of(1, 9, 5)];
        let split = split_per_user(&events, 0.01);

        // user 0 still gets one test event, user 1 (a single event) gets none
        assert_eq!(split.test.len(), 1);
        assert_eq!(split.test[0].track_index, 2);
        assert!(split.train.iter().any(|e| e.user_index == 1));
    }
}