use std::sync::{Arc, RwLock};

use crate::db::error::DbError;
use crate::db::store::Store;
use crate::engine::{EngineConfig, assemble};
use crate::models::adapters::TrendsGenerator;
use crate::ranker::Ranker;

/// Shared by every request. Cloning is cheap: the store and the ranker are shared.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub ranker: Arc<RwLock<Ranker>>,
}

impl AppState {
    pub fn new(store: Store, ranker: Ranker) -> Self {
        Self {
            store,
            ranker: Arc::new(RwLock::new(ranker)),
        }
    }
}

/// Trains every model on the stored data. Training is CPU work, so it runs on
/// a blocking thread and does not stall the async ones.
pub async fn train(store: &Store, config: EngineConfig) -> Result<Ranker, DbError> {
    let events = store.all_events().await?;
    let sessions = store.all_sessions().await?;
    let trending = store
        .trending(config.trend_window, config.trend_pool)
        .await?;

    tokio::task::spawn_blocking(move || {
        let trends = TrendsGenerator::from_ranked(trending);
        assemble(trends, &events, &sessions, &config)
    })
    .await
    .map_err(|error| DbError::Storage(format!("training task failed: {error}")))
}