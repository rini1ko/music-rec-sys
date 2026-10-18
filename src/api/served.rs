//! Which model put a track on the list a listener was shown (R3). The list is
//! ranked when it is served, so that is the moment to note who proposed each
//! track: later the listener has heard some of those tracks and a new ranking
//! would not find them, and the weights of a bandit draw are gone.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::cache::LruCache;
use crate::models::candidate::Source;

const LOCK: &str = "the served lock is never poisoned";
/// Listeners whose lists are remembered; the least recent ones are dropped.
const MAX_LISTENERS: usize = 10_000;
/// Tracks remembered for one listener; more start the notes of that listener afresh.
const MAX_TRACKS_PER_LISTENER: usize = 1_000;

/// `None` as the source means that no model (only rules) proposed the track.
type Credits = HashMap<u32, Option<Source>>;

#[derive(Debug)]
pub struct ServedSources {
    listeners: Mutex<LruCache<u32, Credits>>,
}

impl Default for ServedSources {
    fn default() -> Self {
        Self::new()
    }
}

impl ServedSources {
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(LruCache::new(MAX_LISTENERS)),
        }
    }

    /// Notes the credited model of every track of a list served to `user`.
    pub fn record(&self, user: u32, list: impl IntoIterator<Item = (u32, Option<Source>)>) {
        let list: Vec<(u32, Option<Source>)> = list.into_iter().collect();
        let mut listeners = self.listeners.lock().expect(LOCK);
        let mut credits = listeners.get(&user).unwrap_or_default();
        if credits.len() + list.len() > MAX_TRACKS_PER_LISTENER {
            credits.clear();
        }
        credits.extend(list);
        listeners.insert(user, credits);
    }

    /// The model credited with `track` on a list served to `user`, `None` if
    /// the track was never on such a list or no model proposed it.
    pub fn credit(&self, user: u32, track: u32) -> Option<Source> {
        self.listeners
            .lock()
            .expect(LOCK)
            .get(&user)?
            .get(&track)
            .copied()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_list_decides_who_is_credited() {
        let served = ServedSources::new();
        assert_eq!(served.credit(1, 12), None);

        served.record(1, [(12, Some(Source::Cf)), (13, None)]);
        assert_eq!(served.credit(1, 12), Some(Source::Cf));
        assert_eq!(served.credit(1, 13), None);
        assert_eq!(served.credit(2, 12), None);

        served.record(1, [(12, Some(Source::Markov))]);
        assert_eq!(served.credit(1, 12), Some(Source::Markov));
        // the older list is still remembered
        served.record(1, [(14, None)]);
        assert_eq!(served.credit(1, 12), Some(Source::Markov));
    }

    #[test]
    fn one_listener_cannot_grow_without_limit() {
        let served = ServedSources::new();
        let many = (0..MAX_TRACKS_PER_LISTENER as u32).map(|track| (track, Some(Source::Cf)));
        served.record(1, many);
        served.record(1, [(5_000, Some(Source::Mf))]);

        assert_eq!(served.credit(1, 5_000), Some(Source::Mf));
        assert_eq!(served.credit(1, 0), None);
    }
}
