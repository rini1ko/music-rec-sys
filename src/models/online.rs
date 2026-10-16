//! Plays that arrived after the last training (R1). `POST /events` adds to
//! these counts and the Markov and trends generators add them to what they
//! learned, so the next track and the trends react at once. Co-listening and
//! CF still wait for the next full training.
//!
//! The counts sit behind their own lock, held only for a few map operations.
//! Ranking reads them without waiting for a training, and a training never
//! blocks the writes.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{Duration, NaiveDateTime};

use crate::domain::ListenEvent;

const LOCK: &str = "the online counts lock is never poisoned";

#[derive(Debug, Default)]
struct Counts {
    /// previous track -> next track -> times
    first_order: HashMap<u32, HashMap<u32, u32>>,
    /// (track before the previous, previous) -> next track -> times
    second_order: HashMap<(u32, u32), HashMap<u32, u32>>,
    /// track -> plays
    plays: HashMap<u32, usize>,
    events: usize,
}

/// New plays since the models were trained. Cheap to share: wrap it in an `Arc`.
#[derive(Debug, Default)]
pub struct OnlineCounts {
    counts: RwLock<Counts>,
}

impl OnlineCounts {
    /// Records a play. `previous` holds the tracks played before it in the same
    /// session, oldest first; only the last two matter.
    pub fn record(&self, previous: &[u32], track: u32) {
        let mut counts = self.counts.write().expect(LOCK);
        counts.events += 1;
        *counts.plays.entry(track).or_default() += 1;
        if let [.., before] = previous {
            *counts
                .first_order
                .entry(*before)
                .or_default()
                .entry(track)
                .or_default() += 1;
        }
        if let [.., two_back, before] = previous {
            *counts
                .second_order
                .entry((*two_back, *before))
                .or_default()
                .entry(track)
                .or_default() += 1;
        }
    }

    /// Plays recorded since the last training.
    pub fn events(&self) -> usize {
        self.counts.read().expect(LOCK).events
    }

    /// Adds the new transitions after `track` to `into`.
    pub fn add_next_after(&self, track: u32, into: &mut HashMap<u32, u32>) {
        let counts = self.counts.read().expect(LOCK);
        if let Some(next) = counts.first_order.get(&track) {
            merge(into, next);
        }
    }

    /// Adds the new transitions after the pair `(two_back, previous)` to `into`.
    pub fn add_next_after_pair(&self, pair: (u32, u32), into: &mut HashMap<u32, u32>) {
        let counts = self.counts.read().expect(LOCK);
        if let Some(next) = counts.second_order.get(&pair) {
            merge(into, next);
        }
    }

    /// Adds the new plays to `into`, track by track.
    pub fn add_plays(&self, into: &mut HashMap<u32, usize>) {
        let counts = self.counts.read().expect(LOCK);
        for (&track, &plays) in &counts.plays {
            *into.entry(track).or_default() += plays;
        }
    }
}

/// The tracks before a play at `at` that belong to its session, oldest first.
/// `recent` are the user's latest plays before it, newest first; the chain
/// stops at the first pause longer than `gap`, as the sessionizer does.
pub fn session_tail(recent: &[ListenEvent], at: NaiveDateTime, gap: Duration) -> Vec<u32> {
    let mut tail = Vec::new();
    let mut later = at;
    for event in recent {
        if event.timestamp > later || later - event.timestamp > gap {
            break;
        }
        tail.push(event.track_index);
        later = event.timestamp;
    }
    tail.reverse();
    tail
}

fn merge(into: &mut HashMap<u32, u32>, from: &HashMap<u32, u32>) {
    for (&track, &times) in from {
        *into.entry(track).or_default() += times;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event;

    #[test]
    fn only_the_current_session_is_taken() {
        let gap = Duration::minutes(30);
        // newest first: 5 min, 20 min and 100 min into the data
        let recent = [event(3, 20), event(2, 5), event(1, -100)];
        let at = event(0, 40).timestamp;
        assert_eq!(session_tail(&recent, at, gap), vec![2, 3]);

        // a long pause before the new play: a new session, nothing before it
        let late = event(0, 200).timestamp;
        assert!(session_tail(&recent, late, gap).is_empty());
    }

    #[test]
    fn plays_and_transitions_are_counted() {
        let online = OnlineCounts::default();
        online.record(&[], 1);
        online.record(&[1], 2);
        online.record(&[5, 1], 2);

        assert_eq!(online.events(), 3);
        let mut plays = HashMap::from([(2, 10)]);
        online.add_plays(&mut plays);
        assert_eq!(plays, HashMap::from([(1, 1), (2, 12)]));

        let mut next = HashMap::new();
        online.add_next_after(1, &mut next);
        assert_eq!(next, HashMap::from([(2, 2)]));

        let mut after_pair = HashMap::new();
        online.add_next_after_pair((5, 1), &mut after_pair);
        assert_eq!(after_pair, HashMap::from([(2, 1)]));
        online.add_next_after_pair((9, 9), &mut after_pair);
        assert_eq!(after_pair.len(), 1);
    }
}
