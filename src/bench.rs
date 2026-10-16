//! Measurements for the `simulate` and `bench` commands (R6): latency
//! percentiles, a load simulation through the async `Store`, and the time each
//! model takes to train and to rank.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use chrono::Duration as Span;

use crate::db::error::DbError;
use crate::db::store::Store;
use crate::domain::{ListenEvent, Session};
use crate::engine::{EngineConfig, assemble};
use crate::etl::sessionizer::sessionize_all;
use crate::models::adapters::TrendsGenerator;
use crate::models::candidate::RecommendationContext;
use crate::models::cf::CfModel;
use crate::models::co_listening::CoListeningGraph;
use crate::models::markov::MarkovModel;
use crate::rng::Rng;

/// Collected durations of one kind of operation.
#[derive(Debug, Default, Clone)]
pub struct Latencies {
    samples: Vec<Duration>,
}

/// Percentiles of a set of durations (nearest-rank method).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencySummary {
    pub count: usize,
    pub mean: Duration,
    pub p50: Duration,
    pub p90: Duration,
    pub p99: Duration,
    pub max: Duration,
}

impl Latencies {
    pub fn record(&mut self, took: Duration) {
        self.samples.push(took);
    }

    /// `None` when nothing was recorded.
    pub fn summary(&self) -> Option<LatencySummary> {
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let max = *sorted.last()?;
        let total: Duration = sorted.iter().sum();
        Some(LatencySummary {
            count: sorted.len(),
            mean: total / u32::try_from(sorted.len()).unwrap_or(u32::MAX),
            p50: percentile(&sorted, 0.50),
            p90: percentile(&sorted, 0.90),
            p99: percentile(&sorted, 0.99),
            max,
        })
    }
}

/// The smallest value with at least `share` of the values at or below it.
fn percentile(sorted: &[Duration], share: f64) -> Duration {
    let rank = (share * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

impl fmt::Display for LatencySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = |value: Duration| value.as_secs_f64() * 1000.0;
        write!(
            f,
            "n={} mean {:.2} ms  p50 {:.2}  p90 {:.2}  p99 {:.2}  max {:.2}",
            self.count,
            ms(self.mean),
            ms(self.p50),
            ms(self.p90),
            ms(self.p99),
            ms(self.max)
        )
    }
}

/// What a load simulation should do.
#[derive(Debug, Clone, Copy)]
pub struct SimulationPlan {
    pub users: usize,
    pub events: usize,
    pub seed: u64,
}

#[derive(Debug, Default)]
pub struct SimulationReport {
    pub users_created: usize,
    pub events_written: usize,
    pub create_user: Latencies,
    pub write_event: Latencies,
    pub read_history: Latencies,
    pub elapsed: Duration,
}

/// Creates `users` listeners and lets them play `events` random tracks one
/// by one through the `Store`, as the API would, timing every call. The
/// plays are a minute apart after the newest stored event. It writes to the
/// database, so run it on a copy (`data/play.db`).
///
/// # Errors
///
/// Database errors, and [`DbError::Storage`] when the database has no tracks.
pub async fn simulate(store: &Store, plan: SimulationPlan) -> Result<SimulationReport, DbError> {
    let started = Instant::now();
    let mut rng = Rng::seeded(plan.seed);
    let tracks: Vec<u32> = store
        .all_tracks()
        .await?
        .into_iter()
        .map(|track| track.track_index)
        .collect();
    if tracks.is_empty() {
        return Err(DbError::Storage(
            "the database has no tracks, run `etl` first".to_string(),
        ));
    }
    let countries: Vec<String> = store
        .all_users()
        .await?
        .into_iter()
        .filter_map(|user| user.country)
        .collect();
    let mut clock = store
        .latest_event_time()
        .await?
        .unwrap_or_else(|| chrono::Utc::now().naive_utc());

    let mut report = SimulationReport::default();
    let mut users = Vec::with_capacity(plan.users);
    for _ in 0..plan.users {
        let country =
            (!countries.is_empty()).then(|| countries[rng.below(countries.len())].clone());
        let begun = Instant::now();
        let user = store.create_user(country, None, None).await?;
        report.create_user.record(begun.elapsed());
        users.push(user.user_index);
    }
    report.users_created = users.len();

    for _ in 0..plan.events {
        let Some(&user) = users.get(rng.below(users.len().max(1))) else {
            break;
        };
        clock += Span::minutes(1);
        let event = ListenEvent {
            user_index: user,
            track_index: tracks[rng.below(tracks.len())],
            timestamp: clock,
        };
        let begun = Instant::now();
        store.add_events(vec![event]).await?;
        report.write_event.record(begun.elapsed());

        let begun = Instant::now();
        store.history(user).await?;
        report.read_history.record(begun.elapsed());
        report.events_written += 1;
    }
    report.elapsed = started.elapsed();
    Ok(report)
}

#[derive(Debug, Default)]
pub struct BenchReport {
    pub events: usize,
    pub sessions: usize,
    /// Each step of a training, in the order they run.
    pub training: Vec<(&'static str, Duration)>,
    pub ranking: Latencies,
}

fn timed<T>(
    steps: &mut Vec<(&'static str, Duration)>,
    name: &'static str,
    work: impl FnOnce() -> T,
) -> T {
    let begun = Instant::now();
    let result = work();
    steps.push((name, begun.elapsed()));
    result
}

/// Trains every model on `events`, timing each one, then ranks `requests`
/// times for random listeners with their real histories.
pub fn bench_engine(
    events: &[ListenEvent],
    config: &EngineConfig,
    requests: usize,
    seed: u64,
) -> BenchReport {
    let mut report = BenchReport {
        events: events.len(),
        ..BenchReport::default()
    };
    let steps = &mut report.training;
    let sessions: Vec<Session> = timed(steps, "sessions", || {
        sessionize_all(events.to_vec(), config.session_gap)
    });
    timed(steps, "trends", || {
        TrendsGenerator::new(events, config.trend_window, config.trend_pool)
    });
    timed(steps, "markov", || {
        MarkovModel::train_with_order(&sessions, config.markov_order)
    });
    timed(steps, "co_listening", || {
        CoListeningGraph::build(&sessions, &config.co_listening)
    });
    timed(steps, "cf", || CfModel::train(events, &config.cf));
    let trends = TrendsGenerator::new(events, config.trend_window, config.trend_pool);
    let ranker = timed(steps, "all together", || {
        assemble(trends, events, &sessions, config, None)
    });
    report.sessions = sessions.len();

    let mut histories: HashMap<u32, Vec<u32>> = HashMap::new();
    for event in events {
        histories
            .entry(event.user_index)
            .or_default()
            .push(event.track_index);
    }
    let mut users: Vec<&Vec<u32>> = histories.values().collect();
    users.sort_by_key(|history| history.len());
    let mut rng = Rng::seeded(seed);
    for _ in 0..requests {
        let Some(history) = users.get(rng.below(users.len().max(1))) else {
            break;
        };
        let context = RecommendationContext {
            history: history.to_vec(),
        };
        let begun = Instant::now();
        ranker.recommend(&context, 10);
        report.ranking.record(begun.elapsed());
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, track, user};

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn percentiles_use_the_nearest_rank() {
        let mut latencies = Latencies::default();
        for value in 1..=100 {
            latencies.record(ms(value));
        }
        let summary = latencies.summary().unwrap();
        assert_eq!(summary.count, 100);
        assert_eq!(summary.p50, ms(50));
        assert_eq!(summary.p90, ms(90));
        assert_eq!(summary.p99, ms(99));
        assert_eq!(summary.max, ms(100));
        assert_eq!(summary.mean, Duration::from_micros(50_500));
        assert!(Latencies::default().summary().is_none());
    }

    #[test]
    fn one_value_is_every_percentile() {
        let mut latencies = Latencies::default();
        latencies.record(ms(7));
        let summary = latencies.summary().unwrap();
        assert_eq!(
            (summary.p50, summary.p99, summary.max),
            (ms(7), ms(7), ms(7))
        );
        assert!(summary.to_string().contains("p50 7.00"));
    }

    #[test]
    fn the_engine_bench_times_every_model() {
        let events: Vec<ListenEvent> = (0..30)
            .map(|minute| event_of(minute as u32 % 3, minute as u32 % 5, minute))
            .collect();
        let report = bench_engine(&events, &EngineConfig::lenient(), 20, 1);
        let steps: Vec<&str> = report.training.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            steps,
            [
                "sessions",
                "trends",
                "markov",
                "co_listening",
                "cf",
                "all together"
            ]
        );
        assert_eq!(report.ranking.summary().unwrap().count, 20);
        assert_eq!(report.events, 30);
    }

    #[tokio::test]
    async fn a_simulation_writes_and_times_every_play() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(0)]).await.unwrap();
        store
            .add_tracks((1..=4).map(track).collect())
            .await
            .unwrap();

        let plan = SimulationPlan {
            users: 3,
            events: 25,
            seed: 9,
        };
        let report = simulate(&store, plan).await.unwrap();
        assert_eq!((report.users_created, report.events_written), (3, 25));
        assert_eq!(report.write_event.summary().unwrap().count, 25);
        assert_eq!(store.counts().await.unwrap().events, 25);

        let empty = Store::open("sqlite::memory:").await.unwrap();
        assert!(simulate(&empty, plan).await.is_err());
    }
}
