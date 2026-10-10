use crate::domain::ListenEvent;

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
}