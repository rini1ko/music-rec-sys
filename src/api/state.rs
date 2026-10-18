use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, watch};
use tokio::task::JoinError;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::api::dto::{EventDto, WeightsDto};
use crate::api::evaluation::EvaluationStatus;
use crate::api::guard::RateLimiter;
use crate::api::metrics::Metrics;
use crate::api::rec_cache::RecommendationCache;
use crate::api::served::ServedSources;
use crate::api::stream;
use crate::config::ServerSettings;
use crate::db::error::DbError;
use crate::db::store::Store;
use crate::engine::{EngineConfig, Weights, assemble};
use crate::etl::sessionizer::sessionize_all;
use crate::models::adapters::TrendsGenerator;
use crate::models::catalog::Catalog;
use crate::models::online::OnlineCounts;
use crate::recommender::Recommender;

const LOCK: &str = "a lock in the app state is never poisoned";

/// Freshly trained models and what they were trained on.
pub struct Trained {
    pub recommender: Recommender,
    /// Plays recorded after this training, read by its Markov and trends models.
    pub online: Arc<OnlineCounts>,
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
    /// Weights used when a request does not override them.
    pub weights: WeightsDto,
}

impl ModelStatus {
    fn new(trained: &Trained, weights: Weights) -> Self {
        Self {
            training: false,
            trained_at: Utc::now(),
            train_seconds: trained.seconds,
            events: trained.events,
            sessions: trained.sessions,
            last_error: None,
            weights: weights.into(),
        }
    }
}

/// Shared by every request. Cloning is cheap: everything inside is shared.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub recommender: Arc<RwLock<Recommender>>,
    status: Arc<Mutex<ModelStatus>>,
    pub(crate) evaluation: Arc<Mutex<EvaluationStatus>>,
    config: EngineConfig,
    pub(crate) settings: Arc<ServerSettings>,
    pub(crate) metrics: Arc<Metrics>,
    pub(crate) limiter: Arc<RateLimiter>,
    pub(crate) cache: Arc<RecommendationCache>,
    /// Who proposed the tracks of the lists served, for `POST /feedback`.
    pub(crate) served: Arc<ServedSources>,
    online: Arc<Mutex<OnlineSlots>>,
    /// Every stored play, for `GET /events/stream` (R5).
    pub(crate) plays: broadcast::Sender<EventDto>,
    /// Turns `true` when the server starts to shut down, so the open streams end.
    shutdown: Arc<watch::Sender<bool>>,
}

/// Online counts (R1) of the models in use and, while a training runs, of the
/// models being trained, so plays posted meanwhile reach both.
#[derive(Debug, Default)]
struct OnlineSlots {
    current: Arc<OnlineCounts>,
    next: Option<Arc<OnlineCounts>>,
}

impl AppState {
    /// A state with [`ServerSettings::default`]; see [`AppState::with_settings`].
    pub fn new(store: Store, trained: Trained, config: EngineConfig) -> Self {
        let status = ModelStatus::new(&trained, config.weights);
        let settings = ServerSettings::default();
        Self {
            store,
            recommender: Arc::new(RwLock::new(trained.recommender)),
            online: Arc::new(Mutex::new(OnlineSlots {
                current: trained.online,
                next: None,
            })),
            status: Arc::new(Mutex::new(status)),
            evaluation: Arc::new(Mutex::new(EvaluationStatus::default())),
            config,
            limiter: Arc::new(RateLimiter::new(
                settings.rate_per_second,
                settings.rate_burst,
            )),
            cache: Arc::new(RecommendationCache::new(settings.cache_capacity)),
            served: Arc::new(ServedSources::new()),
            metrics: Arc::new(Metrics::default()),
            plays: stream::channel(),
            shutdown: Arc::new(watch::channel(false).0),
            settings: Arc::new(settings),
        }
    }

    /// Tells the open `GET /events/stream` connections to finish. The play
    /// channel cannot do it: its sender lives in this state, so it never closes.
    pub fn begin_shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    /// Watches [`AppState::begin_shutdown`]; a new receiver sees a shutdown
    /// that has already begun.
    pub(crate) fn shutdown_watch(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    /// The same state with other limits, key and cache size.
    #[must_use]
    pub fn with_settings(mut self, settings: ServerSettings) -> Self {
        self.limiter = Arc::new(RateLimiter::new(
            settings.rate_per_second,
            settings.rate_burst,
        ));
        self.cache = Arc::new(RecommendationCache::new(settings.cache_capacity));
        self.settings = Arc::new(settings);
        self
    }

    /// The configuration the models are trained with.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Runs `work` with the trained recommender on a blocking thread: ranking
    /// is CPU work, and the read guard never lives across an `.await`.
    pub async fn rank<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Recommender) -> T + Send + 'static,
    ) -> Result<T, JoinError> {
        let recommender = Arc::clone(&self.recommender);
        tokio::task::spawn_blocking(move || work(&recommender.read().expect(LOCK))).await
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

    /// Counts a new play for the online parts of the models (R1). `previous`
    /// holds the tracks played before it in the same session, oldest first.
    pub fn record_play(&self, previous: &[u32], track: u32) {
        let (current, next) = {
            let slots = self.online.lock().expect(LOCK);
            (Arc::clone(&slots.current), slots.next.clone())
        };
        current.record(previous, track);
        if let Some(next) = next {
            next.record(previous, track);
        }
    }

    /// Plays counted online since the models in use were trained.
    pub fn online_events(&self) -> usize {
        self.online.lock().expect(LOCK).current.events()
    }

    /// Requests keep using the old models until the new ones are ready.
    async fn run_training(&self) {
        // set up before the events are read, so no play falls between the
        // snapshot and the swap (a few may be counted twice, none is lost)
        let next = Arc::new(OnlineCounts::default());
        self.online.lock().expect(LOCK).next = Some(Arc::clone(&next));
        match train_counting(&self.store, self.config.clone(), next).await {
            Ok(trained) => {
                let status = ModelStatus::new(&trained, self.config.weights);
                tracing::info!(
                    seconds = trained.seconds,
                    events = trained.events,
                    sessions = trained.sessions,
                    "models retrained"
                );
                // the old models are dropped after the lock is released
                let old = std::mem::replace(
                    &mut *self.recommender.write().expect(LOCK),
                    trained.recommender,
                );
                drop(old);
                let mut slots = self.online.lock().expect(LOCK);
                if let Some(next) = slots.next.take() {
                    slots.current = next;
                }
                drop(slots);
                // lists ranked by the old models must not outlive them
                self.cache.clear();
                *self.status.lock().expect(LOCK) = status;
            }
            Err(error) => {
                tracing::error!(%error, "retraining failed, keeping the old models");
                self.online.lock().expect(LOCK).next = None;
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
    train_counting(store, config, Arc::default()).await
}

/// Like [`train`], with the online counts the new models should read.
pub async fn train_counting(
    store: &Store,
    config: EngineConfig,
    online: Arc<OnlineCounts>,
) -> Result<Trained, DbError> {
    let started = Instant::now();
    let events = store.all_events().await?;
    let tracks = store.all_tracks().await?;
    let users = store.all_users().await?;
    let trending = store
        .trending(config.trend_window, config.trend_pool)
        .await?;

    tokio::task::spawn_blocking(move || {
        let sessions = sessionize_all(events.clone(), config.session_gap);
        let trends = TrendsGenerator::from_ranked(trending);
        let ranker = assemble(
            trends,
            &events,
            &sessions,
            &config,
            Some(Arc::clone(&online)),
        );
        let catalog = Catalog::build(&tracks, &users, &events, config.recommender.cold_start_pool);

        Trained {
            recommender: Recommender::new(ranker, catalog, config.recommender.clone()),
            online,
            events: events.len(),
            sessions: sessions.len(),
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
