//! Ready recommendation lists (B3). A list depends on the user, the length
//! and the weights of the request, and on the user's history and the models,
//! so a new play of the user drops their lists and a retraining drops all.

use std::sync::Mutex;

use crate::api::dto::RecommendationsResponse;
use crate::cache::LruCache;
use crate::recommender::WeightOverrides;

const LOCK: &str = "the cache lock is never poisoned";

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
    lists: Mutex<LruCache<CacheKey, RecommendationsResponse>>,
}

impl RecommendationCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            lists: Mutex::new(LruCache::new(capacity)),
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<RecommendationsResponse> {
        self.lists.lock().expect(LOCK).get(key)
    }

    pub fn insert(&self, key: CacheKey, list: RecommendationsResponse) {
        self.lists.lock().expect(LOCK).insert(key, list);
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

    use crate::api::router;
    use crate::api::test_support::{send, state};

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
