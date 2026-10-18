//! Ready recommendation lists (B3). A list depends on the user, the length
//! and the weights of the request, and on the user's history and the models,
//! so a new play of the user drops their lists and a retraining drops all.
//! A play also changes the shared online counts (R1) that every list is ranked
//! from, so a list is kept only for [`LIST_TTL`]: other listeners see what the
//! models learned after a short while, not only after a retraining.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::api::dto::RecommendationsResponse;
use crate::cache::LruCache;
use crate::recommender::WeightOverrides;

const LOCK: &str = "the cache lock is never poisoned";

/// How long a ready list is served before it is ranked again.
pub const LIST_TTL: Duration = Duration::from_secs(30);

/// Weights are kept as bits: `f64` is neither `Eq` nor `Hash`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    user: u32,
    top: usize,
    weights: [Option<u64>; 5],
}

impl CacheKey {
    pub fn new(user: u32, top: usize, overrides: &WeightOverrides) -> Self {
        let bits = |weight: Option<f64>| weight.map(f64::to_bits);
        Self {
            user,
            top,
            weights: [
                bits(overrides.trends),
                bits(overrides.markov),
                bits(overrides.co_listening),
                bits(overrides.cf),
                bits(overrides.mf),
            ],
        }
    }
}

#[derive(Debug)]
pub struct RecommendationCache {
    lists: Mutex<LruCache<CacheKey, (Instant, RecommendationsResponse)>>,
    ttl: Duration,
}

impl RecommendationCache {
    pub fn new(capacity: usize) -> Self {
        Self::with_ttl(capacity, LIST_TTL)
    }

    /// A cache whose lists expire after `ttl`.
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            lists: Mutex::new(LruCache::new(capacity)),
            ttl,
        }
    }

    /// The list, unless it is older than the time to live.
    pub fn get(&self, key: &CacheKey) -> Option<RecommendationsResponse> {
        let mut lists = self.lists.lock().expect(LOCK);
        let (stored, list) = lists.get(key)?;
        if stored.elapsed() < self.ttl {
            return Some(list);
        }
        lists.remove_where(|other| other == key);
        None
    }

    pub fn insert(&self, key: CacheKey, list: RecommendationsResponse) {
        self.lists
            .lock()
            .expect(LOCK)
            .insert(key, (Instant::now(), list));
    }

    /// The user played something: their lists are out of date.
    pub fn forget_user(&self, user: u32) {
        self.lists
            .lock()
            .expect(LOCK)
            .remove_where(|key| key.user == user);
    }

    /// New models: every list is out of date.
    pub fn clear(&self) {
        self.lists.lock().expect(LOCK).clear();
    }

    /// Hits, misses and the number of kept lists.
    pub fn stats(&self) -> (u64, u64, usize) {
        let lists = self.lists.lock().expect(LOCK);
        let (hits, misses) = lists.stats();
        (hits, misses, lists.len())
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    use crate::api::router;
    use crate::api::test_support::{send, state};

    #[test]
    fn a_list_expires_after_its_time_to_live() {
        let list = RecommendationsResponse {
            user: 1,
            history_len: 0,
            items: Vec::new(),
            bandit_weights: None,
        };
        let key = CacheKey::new(1, 3, &WeightOverrides::default());

        let fresh = RecommendationCache::with_ttl(4, Duration::from_secs(60));
        fresh.insert(key.clone(), list.clone());
        assert!(fresh.get(&key).is_some());

        let stale = RecommendationCache::with_ttl(4, Duration::ZERO);
        stale.insert(key.clone(), list);
        assert!(stale.get(&key).is_none());
        assert_eq!(stale.stats().2, 0);
    }

    #[tokio::test]
    async fn repeated_requests_come_from_the_cache_until_the_user_plays() {
        let state = state().await;
        let app = router(state.clone());

        let (_, first) = send(&app, "GET", "/recommendations?user=1&top=3", None).await;
        let (_, second) = send(&app, "GET", "/recommendations?user=1&top=3", None).await;
        assert_eq!(first, second);
        assert_eq!(state.cache.stats(), (1, 1, 1));

        // other weights are another list
        send(&app, "GET", "/recommendations?user=1&top=3&cf=0", None).await;
        assert_eq!(state.cache.stats().2, 2);

        let (status, _) = send(&app, "POST", "/events", Some(r#"{"user":1,"track":12}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(state.cache.stats().2, 0);

        let (_, after) = send(&app, "GET", "/recommendations?user=1&top=3", None).await;
        assert_eq!(after["history_len"], 3);
    }
}
