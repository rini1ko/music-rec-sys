//! A small least-recently-used cache. The API keeps ready recommendation
//! lists in it (B3), so a page reload does not rank the same user again.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// Keeps at most `capacity` values and drops the one used longest ago.
///
/// Every access gets a number from a counter; `order` maps those numbers to
/// keys, so the oldest entry is the first one in the map. Reads and writes
/// cost `O(log n)`, which is plenty for a few thousand entries.
#[derive(Debug)]
pub struct LruCache<K, V> {
    capacity: usize,
    tick: u64,
    entries: HashMap<K, (V, u64)>,
    order: BTreeMap<u64, K>,
    hits: u64,
    misses: u64,
}

impl<K: Eq + Hash + Clone, V: Clone> LruCache<K, V> {
    /// A capacity of 0 turns the cache off: nothing is ever stored.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            tick: 0,
            entries: HashMap::with_capacity(capacity.min(4096)),
            order: BTreeMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// The value, marked as the most recently used one.
    pub fn get(&mut self, key: &K) -> Option<V> {
        let tick = self.next_tick();
        let Some((value, used)) = self.entries.get_mut(key) else {
            self.misses += 1;
            return None;
        };
        self.order.remove(used);
        *used = tick;
        self.order.insert(tick, key.clone());
        self.hits += 1;
        Some(value.clone())
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }
        let tick = self.next_tick();
        if let Some((_, old)) = self.entries.insert(key.clone(), (value, tick)) {
            self.order.remove(&old);
        }
        self.order.insert(tick, key);
        while self.entries.len() > self.capacity {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    /// Drops every entry whose key matches, e.g. all lists of one user.
    pub fn remove_where(&mut self, matches: impl Fn(&K) -> bool) {
        self.entries.retain(|key, _| !matches(key));
        self.order.retain(|_, key| !matches(key));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Reads that found a value and reads that did not, since the start.
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_recently_used_entry_goes_first() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        // reading "a" makes "b" the oldest
        assert_eq!(cache.get(&"a"), Some(1));
        cache.insert("c", 3);

        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"c"), Some(3));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn inserting_an_existing_key_replaces_the_value() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("a", 2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&"a"), Some(2));
        assert_eq!(cache.order.len(), 1);
    }

    #[test]
    fn entries_can_be_dropped_by_key() {
        let mut cache = LruCache::new(10);
        cache.insert((1, 10), "x");
        cache.insert((1, 20), "y");
        cache.insert((2, 10), "z");
        cache.remove_where(|(user, _)| *user == 1);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.order.len(), 1);
        assert_eq!(cache.get(&(2, 10)), Some("z"));
    }

    #[test]
    fn hits_and_misses_are_counted() {
        let mut cache = LruCache::new(1);
        cache.insert(1, 1);
        cache.get(&1);
        cache.get(&2);
        assert_eq!(cache.stats(), (1, 1));
    }

    #[test]
    fn zero_capacity_stores_nothing() {
        let mut cache = LruCache::new(0);
        cache.insert(1, 1);
        assert!(cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }
}
