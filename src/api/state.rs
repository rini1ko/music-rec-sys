use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::error::DbError;
use crate::db::store::Store;
use crate::engine::{EngineConfig, assemble};
use crate::etl::sessionizer::sessionize_all;
use crate::models::adapters::TrendsGenerator;
use crate::ranker::Ranker;

const LOCK: &str = "a lock in the app state is never poisoned";

/// Freshly trained models and what they were trained on.
pub struct Trained {
    pub ranker: Ranker,
    pub events: usize,
    pub sessions: usize,
    pub seconds: f32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelStatus {
    pub training: bool,
    pub trained_at: DateTime<Utc>,
    pub train_seconds: f32,
    pub events: usize,
    pub sessions: usize,
    pub last_error: Option<String>,
}

impl ModelStatus {
    fn new(trained: &Trained) -> Self {
        Self {
            training: false,
            trained_at: Utc::now(),
            train_seconds: trained.seconds,
            events: trained.events,
            sessions: trained.sessions,
            last_error: None,
        }
    }
}

/// Shared by every request. Cloning is cheap: everything inside is shared.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub ranker: Arc<RwLock<Ranker>>,
    status: Arc<Mutex<ModelStatus>>,
    config: EngineConfig,
}

impl AppState {
    pub fn new(store: Store, trained: Trained, config: EngineConfig) -> Self {
        let status = ModelStatus::new(&trained);
        Self {
            store,
            ranker: Arc::new(RwLock::new(trained.ranker)),
            status: Arc::new(Mutex::new(status)),
            config,
        }
    }

    pub fn status(&self) -> ModelStatus {
        self.status.lock().expect(LOCK).clone()
    }

    /// Starts retraining in the background. False means one is already running.
    pub fn retrain(&self) -> bool {
        if !self.try_begin() {
            return false;
        }
        let state = self.clone();
        tokio::spawn(async move { state.run_training().await });
        true
    }

    fn try_begin(&self) -> bool {
        let mut status = self.status.lock().expect(LOCK);
        if status.training {
            return false;
        }
        status.training = true;
        true
    }

    /// Requests keep using the old ranker until the new one is ready.
    async fn run_training(&self) {
        match train(&self.store, self.config.clone()).await {
            Ok(trained) => {
                let status = ModelStatus::new(&trained);
                tracing::info!(
                    seconds = trained.seconds,
                    events = trained.events,
                    sessions = trained.sessions,
                    "models retrained"
                );
                // the old models are dropped after the lock is released
                let old = std::mem::replace(&mut *self.ranker.write().expect(LOCK), trained.ranker);
                drop(old);
                *self.status.lock().expect(LOCK) = status;
            }
            Err(error) => {
                tracing::error!(%error, "retraining failed, keeping the old models");
                let mut status = self.status.lock().expect(LOCK);
                status.training = false;
                status.last_error = Some(error.to_string());
            }
        }
    }
}

/// Trains every model on the stored events. Sessions are cut from the events
/// again, so events added after the import are part of them. Training is CPU
/// work, so it runs on a blocking thread and does not stall the async ones.
pub async fn train(store: &Store, config: EngineConfig) -> Result<Trained, DbError> {
    let started = Instant::now();
    let events = store.all_events().await?;
    let trending = store
        .trending(config.trend_window, config.trend_pool)
        .await?;

    tokio::task::spawn_blocking(move || {
        let sessions = sessionize_all(events.clone(), config.session_gap);
        let trends = TrendsGenerator::from_ranked(trending);
        let ranker = assemble(trends, &events, &sessions, &config);
        Trained {
            events: events.len(),
            sessions: sessions.len(),
            ranker,
            seconds: started.elapsed().as_secs_f32(),
        }
    })
    .await
    .map_err(|error| DbError::Storage(format!("training task failed: {error}")))
}

/// Retrains on a timer. A tick that finds a training still running is skipped.
/// `every` must not be zero.
pub fn spawn_schedule(state: AppState, every: Duration) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(every);
        // the first tick fires at once, and the models were just trained
        timer.tick().await;
        loop {
            timer.tick().await;
            if !state.retrain() {
                tracing::warn!("scheduled retraining skipped, the previous one is still running");
            }
        }
    });
}