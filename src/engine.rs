use std::fmt;

use chrono::Duration;

use crate::db::error::DbError;
use crate::db::repository::Repository;
use crate::domain::{ListenEvent, Session};
use crate::models::adapters::{
    CfGenerator, CoListeningGenerator, MarkovGenerator, TrendsGenerator,
};
use crate::models::cf::{CfConfig, CfModel};
use crate::models::co_listening::{CoListeningConfig, CoListeningGraph};
use crate::models::markov::MarkovModel;
use crate::ranker::{Ranker, RankerConfig};
use crate::recommender::RecommenderConfig;

#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub trends: f64,
    pub markov: f64,
    pub co_listening: f64,
    pub cf: f64,
}

impl Weights {
    /// Every model switched off. Handy as a base: `Weights { cf: 1.0, ..Weights::ZERO }`.
    pub const ZERO: Weights = Weights {
        trends: 0.0,
        markov: 0.0,
        co_listening: 0.0,
        cf: 0.0,
    };
}

impl fmt::Display for Weights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trends {:.1} / markov {:.1} / co-listening {:.1} / cf {:.1}",
            self.trends, self.markov, self.co_listening, self.cf
        )
    }
}

#[derive(Clone)]
pub struct EngineConfig {
    pub trend_window: Duration,
    pub trend_pool: usize,
    pub session_gap: Duration,
    pub co_listening: CoListeningConfig,
    pub cf: CfConfig,
    pub co_listening_recent_tracks: usize,
    pub candidates_per_source: usize,
    pub weights: Weights,
    pub recommender: RecommenderConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            trend_window: Duration::days(1),
            trend_pool: 50,
            session_gap: Duration::minutes(30),
            co_listening: CoListeningConfig::default(),
            cf: CfConfig::default(),
            co_listening_recent_tracks: 5,
            candidates_per_source: 50,
            weights: Weights {
                trends: 0.1,
                markov: 0.2,
                co_listening: 0.4,
                cf: 0.3,
            },
            recommender: RecommenderConfig::default(),
        }
    }
}

impl EngineConfig {
    /// Same config with different model weights (used to compare models).
    pub fn with_weights(mut self, weights: Weights) -> Self {
        self.weights = weights;
        self
    }

    /// Small datasets do not have enough overlap for the default thresholds.
    pub fn lenient() -> Self {
        EngineConfig {
            co_listening: CoListeningConfig {
                min_cooccurrence: 1,
                min_jaccard: 0.0,
                ..CoListeningConfig::default()
            },
            cf: CfConfig {
                min_overlap: 1,
                ..CfConfig::default()
            },
            ..EngineConfig::default()
        }
    }
}

/// Trains every model on the data in the repository and wires them into a ranker.
/// Trends are counted by the repository itself, the other models need all the data.
pub fn build_ranker(
    repo: &(impl Repository + ?Sized),
    config: &EngineConfig,
) -> Result<Ranker, DbError> {
    let events = repo.all_events()?;
    let sessions = repo.all_sessions()?;
    let ranked = repo.trending(config.trend_window, config.trend_pool)?;
    Ok(assemble(
        TrendsGenerator::from_ranked(ranked),
        &events,
        &sessions,
        config,
    ))
}

/// Same as `build_ranker`, but trains on data that is already in memory.
/// Evaluation uses it to train on a slice of the data without a repository.
pub fn build_ranker_from_data(
    events: &[ListenEvent],
    sessions: &[Session],
    config: &EngineConfig,
) -> Ranker {
    let trends = TrendsGenerator::new(events, config.trend_window, config.trend_pool);
    assemble(trends, events, sessions, config)
}

pub fn assemble(
    trends: TrendsGenerator,
    events: &[ListenEvent],
    sessions: &[Session],
    config: &EngineConfig,
) -> Ranker {
    let markov = MarkovGenerator::new(MarkovModel::train(sessions));
    let graph = CoListeningGraph::build(sessions, &config.co_listening);
    let co_listening = CoListeningGenerator::new(graph, config.co_listening_recent_tracks);
    let cf = CfGenerator::new(CfModel::train(events, &config.cf));

    Ranker::new(RankerConfig {
        candidates_per_source: config.candidates_per_source,
    })
    .with_generator(Box::new(trends), config.weights.trends)
    .with_generator(Box::new(markov), config.weights.markov)
    .with_generator(Box::new(co_listening), config.weights.co_listening)
    .with_generator(Box::new(cf), config.weights.cf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory::MemoryRepository;
    use crate::domain::test_utils::{event_of, session_of, track, user};
    use crate::models::candidate::RecommendationContext;

    fn lenient_config() -> EngineConfig {
        EngineConfig {
            co_listening: CoListeningConfig {
                min_cooccurrence: 1,
                min_jaccard: 0.0,
                ..CoListeningConfig::default()
            },
            cf: CfConfig {
                min_overlap: 1,
                ..CfConfig::default()
            },
            ..EngineConfig::default()
        }
    }

    fn sample_repo() -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.add_users(vec![user(0), user(1), user(2)]).unwrap();
        repo.add_tracks(vec![track(1), track(2)]).unwrap();
        repo.add_events(vec![
            event_of(0, 1, 0),
            event_of(0, 2, 1),
            event_of(1, 1, 100),
            event_of(1, 2, 101),
            event_of(2, 1, 200),
        ])
        .unwrap();
        repo.add_sessions(vec![
            session_of(0, &[1, 2]),
            session_of(1, &[1, 2]),
            session_of(2, &[1]),
        ])
        .unwrap();
        repo
    }

    #[test]
    fn ranker_built_from_repository_recommends_unheard_tracks() {
        let repo = sample_repo();
        let ranker = build_ranker(&repo, &lenient_config()).unwrap();
        let context = RecommendationContext {
            history: repo.user_history(2).unwrap(),
        };

        let recommendations = ranker.recommend(&context, 5);

        // user 2 only heard track 1, so track 2 is the only candidate left;
        // all four models vote for it: 0.1 + 0.2 + 0.4 + 0.3
        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].track_index, 2);
        assert!((recommendations[0].score - 1.0).abs() < 1e-9);
        assert_eq!(recommendations[0].reasons.len(), 4);
    }

    #[test]
    fn ranker_built_from_empty_repository_recommends_nothing() {
        let repo = MemoryRepository::default();
        let ranker = build_ranker(&repo, &EngineConfig::default()).unwrap();
        let context = RecommendationContext {
            history: vec![1, 2],
        };

        assert!(ranker.recommend(&context, 5).is_empty());
    }
}