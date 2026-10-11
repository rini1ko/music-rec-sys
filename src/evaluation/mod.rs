pub mod metrics;
pub mod split;
pub mod tuning;

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use chrono::Duration;
use thiserror::Error;

use crate::domain::ListenEvent;
use crate::engine::{EngineConfig, Weights, build_ranker_from_data};
use crate::etl::sessionizer::sessionize_all;
use crate::models::candidate::{RecommendationContext, Source};
use crate::ranker::Ranker;

use metrics::{hit_at_k, ndcg_at_k, precision_at_k, recall_at_k};
use split::{SplitMode, split};

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("test_fraction must be strictly between 0 and 1, got {0}")]
    InvalidFraction(f64),
    #[error("k must be at least 1")]
    ZeroK,
    #[error("there are no events to evaluate on")]
    NoEvents,
}

pub struct EvalConfig {
    /// Length of the recommendation list that is judged.
    pub k: usize,
    /// Share of the NEWEST events that is held out for testing.
    pub test_fraction: f64,
    /// Pause that splits training events into sessions.
    pub session_gap: Duration,
    pub split: SplitMode,
}

impl Default for EvalConfig {
    fn default() -> Self {
        EvalConfig {
            k: 10,
            test_fraction: 0.2,
            session_gap: Duration::minutes(30),
            split: SplitMode::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalReport {
    pub k: usize,
    pub train_events: usize,
    pub test_events: usize,
    pub users_evaluated: usize,
    /// Users that appear in the test part but have no training history.
    pub skipped_cold_start: usize,
    /// Users whose test tracks were all heard in training already.
    pub skipped_nothing_new: usize,
    /// Share of users with at least one relevant track in the top-k.
    pub hit_rate: f64,
    pub precision: f64,
    pub recall: f64,
    pub ndcg: f64,
}

impl fmt::Display for EvalReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hit@{k} {:.4}  precision@{k} {:.4}  recall@{k} {:.4}  ndcg@{k} {:.4}  (users: {})",
            self.hit_rate,
            self.precision,
            self.recall,
            self.ndcg,
            self.users_evaluated,
            k = self.k
        )
    }
}

/// One user to judge: what we know about them and what they listened to later.
struct Case {
    history: Vec<u32>,
    relevant: HashSet<u32>,
}

/// Trains the models ONCE on the older part of the events and keeps the
/// prepared test cases. Afterwards the weights can be changed and the
/// evaluation repeated cheaply, which is what weight tuning needs.
pub struct Evaluator {
    k: usize,
    ranker: Ranker,
    cases: Vec<Case>,
    train_events: usize,
    test_events: usize,
    skipped_cold_start: usize,
    skipped_nothing_new: usize,
}

impl Evaluator {
    pub fn new(
        events: &[ListenEvent],
        eval: &EvalConfig,
        engine: &EngineConfig,
    ) -> Result<Self, EvalError> {
        if !(eval.test_fraction > 0.0 && eval.test_fraction < 1.0) {
            return Err(EvalError::InvalidFraction(eval.test_fraction));
        }
        if eval.k == 0 {
            return Err(EvalError::ZeroK);
        }
        if events.is_empty() {
            return Err(EvalError::NoEvents);
        }

        let parts = split(events, eval.split, eval.test_fraction);

        // everything below uses ONLY training data, models never see the test part
        let train_sessions = sessionize_all(parts.train.clone(), eval.session_gap);
        let ranker = build_ranker_from_data(&parts.train, &train_sessions, engine);
        let histories = histories(&parts.train);

        let mut evaluator = Evaluator {
            k: eval.k,
            ranker,
            cases: Vec::new(),
            train_events: parts.train.len(),
            test_events: parts.test.len(),
            skipped_cold_start: 0,
            skipped_nothing_new: 0,
        };

        for (user, test_tracks) in tracks_by_user(&parts.test) {
            let Some(history) = histories.get(&user) else {
                evaluator.skipped_cold_start += 1;
                continue;
            };

            let heard: HashSet<u32> = history.iter().copied().collect();
            let relevant: HashSet<u32> = test_tracks.difference(&heard).copied().collect();
            if relevant.is_empty() {
                evaluator.skipped_nothing_new += 1;
                continue;
            }

            evaluator.cases.push(Case {
                history: history.clone(),
                relevant,
            });
        }

        Ok(evaluator)
    }

    pub fn set_weights(&mut self, weights: Weights) {
        self.ranker.set_weight(Source::Trends, weights.trends);
        self.ranker.set_weight(Source::Markov, weights.markov);
        self.ranker
            .set_weight(Source::CoListening, weights.co_listening);
        self.ranker.set_weight(Source::Cf, weights.cf);
    }

    /// Number of users that can be evaluated.
    pub fn case_count(&self) -> usize {
        self.cases.len()
    }

    pub fn run(&self) -> EvalReport {
        self.run_where(|_| true)
    }

    /// Evaluates only the cases whose position passes `keep`.
    /// Tuning uses it to pick weights on one half of the users and to
    /// check them on the other half.
    pub fn run_where(&self, keep: impl Fn(usize) -> bool) -> EvalReport {
        let mut report = EvalReport {
            k: self.k,
            train_events: self.train_events,
            test_events: self.test_events,
            users_evaluated: 0,
            skipped_cold_start: self.skipped_cold_start,
            skipped_nothing_new: self.skipped_nothing_new,
            hit_rate: 0.0,
            precision: 0.0,
            recall: 0.0,
            ndcg: 0.0,
        };

        for (position, case) in self.cases.iter().enumerate() {
            if !keep(position) {
                continue;
            }

            let context = RecommendationContext {
                history: case.history.clone(),
            };
            let recommended: Vec<u32> = self
                .ranker
                .recommend(&context, self.k)
                .into_iter()
                .map(|r| r.track_index)
                .collect();

            report.users_evaluated += 1;
            report.hit_rate += hit_at_k(&recommended, &case.relevant, self.k);
            report.precision += precision_at_k(&recommended, &case.relevant, self.k);
            report.recall += recall_at_k(&recommended, &case.relevant, self.k);
            report.ndcg += ndcg_at_k(&recommended, &case.relevant, self.k);
        }

        if report.users_evaluated > 0 {
            let users = report.users_evaluated as f64;
            report.hit_rate /= users;
            report.precision /= users;
            report.recall /= users;
            report.ndcg /= users;
        }
        report
    }
}

/// Trains every model on the older part of the events and checks, user by
/// user, how many of the tracks they listened to LATER end up in the top-k.
///
/// Only tracks the user had not heard during training count as relevant,
/// because the ranker never recommends tracks from the user's history.
pub fn evaluate(
    events: &[ListenEvent],
    eval: &EvalConfig,
    engine: &EngineConfig,
) -> Result<EvalReport, EvalError> {
    Ok(Evaluator::new(events, eval, engine)?.run())
}

/// user -> tracks they listened to, oldest first.
fn histories(events: &[ListenEvent]) -> BTreeMap<u32, Vec<u32>> {
    let mut by_user: BTreeMap<u32, Vec<&ListenEvent>> = BTreeMap::new();
    for event in events {
        by_user.entry(event.user_index).or_default().push(event);
    }
    by_user
        .into_iter()
        .map(|(user, mut events)| {
            events.sort_by_key(|e| e.timestamp);
            (user, events.into_iter().map(|e| e.track_index).collect())
        })
        .collect()
}

fn tracks_by_user(events: &[ListenEvent]) -> BTreeMap<u32, HashSet<u32>> {
    let mut by_user: BTreeMap<u32, HashSet<u32>> = BTreeMap::new();
    for event in events {
        by_user
            .entry(event.user_index)
            .or_default()
            .insert(event.track_index);
    }
    by_user
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event_of;
    use crate::models::co_listening::CoListeningConfig;

    fn lenient_engine() -> EngineConfig {
        EngineConfig {
            co_listening: CoListeningConfig {
                min_cooccurrence: 1,
                min_jaccard: 0.0,
                ..CoListeningConfig::default()
            },
            ..EngineConfig::default()
        }
    }

    fn global(k: usize, test_fraction: f64) -> EvalConfig {
        EvalConfig {
            k,
            test_fraction,
            split: SplitMode::Global,
            ..EvalConfig::default()
        }
    }

    /// Four users listen to 1 -> 2 -> 3 in the past. The newest events are
    /// user 4 and user 5 listening to 3 after 1 and 2: the models should predict it.
    fn pattern_events() -> Vec<ListenEvent> {
        let mut events = Vec::new();
        for user in 0..4 {
            let base = i64::from(user) * 100;
            events.push(event_of(user, 1, base));
            events.push(event_of(user, 2, base + 1));
            events.push(event_of(user, 3, base + 2));
        }
        // training history of the evaluated users
        events.push(event_of(4, 1, 500));
        events.push(event_of(4, 2, 501));
        events.push(event_of(5, 1, 510));
        events.push(event_of(5, 2, 511));
        // the held-out future
        events.push(event_of(4, 3, 1000));
        events.push(event_of(5, 3, 1010));
        events
    }

    #[test]
    fn model_that_learned_the_pattern_scores_high() {
        let report =
            evaluate(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine()).unwrap();

        assert_eq!(report.test_events, 2);
        assert_eq!(report.users_evaluated, 2);
        assert!((report.hit_rate - 1.0).abs() < 1e-9);
        assert!((report.recall - 1.0).abs() < 1e-9);
        assert!((report.ndcg - 1.0).abs() < 1e-9);
    }

    #[test]
    fn user_without_training_history_is_counted_as_cold_start() {
        let mut events = pattern_events();
        events.push(event_of(99, 3, 2000));
        let report = evaluate(&events, &global(3, 3.0 / 19.0), &lenient_engine()).unwrap();

        assert_eq!(report.skipped_cold_start, 1);
        assert_eq!(report.users_evaluated, 2);
    }

    #[test]
    fn already_heard_tracks_are_not_relevant() {
        let events = vec![
            event_of(0, 1, 0),
            event_of(0, 2, 1),
            event_of(0, 1, 100), // a repeat of a track from training
        ];
        // only the last event is held out
        let report = evaluate(&events, &global(10, 0.2), &lenient_engine()).unwrap();

        assert_eq!(report.users_evaluated, 0);
        assert_eq!(report.skipped_nothing_new, 1);
    }

    #[test]
    fn model_weights_change_the_result() {
        let eval = global(3, 2.0 / 18.0);
        let markov_only = lenient_engine().with_weights(Weights {
            markov: 1.0,
            ..Weights::ZERO
        });
        let nothing = lenient_engine().with_weights(Weights::ZERO);

        let with_model = evaluate(&pattern_events(), &eval, &markov_only).unwrap();
        let without = evaluate(&pattern_events(), &eval, &nothing).unwrap();

        assert!(with_model.ndcg > without.ndcg);
        assert_eq!(without.ndcg, 0.0);
    }

    #[test]
    fn weights_can_be_changed_after_training() {
        let mut evaluator =
            Evaluator::new(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine()).unwrap();

        evaluator.set_weights(Weights::ZERO);
        assert_eq!(evaluator.run().ndcg, 0.0);

        evaluator.set_weights(Weights {
            markov: 1.0,
            ..Weights::ZERO
        });
        assert!(evaluator.run().ndcg > 0.9);
    }

    #[test]
    fn run_where_evaluates_only_selected_cases() {
        let evaluator =
            Evaluator::new(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine()).unwrap();

        assert_eq!(evaluator.case_count(), 2);
        assert_eq!(evaluator.run_where(|i| i == 0).users_evaluated, 1);
        assert_eq!(evaluator.run_where(|_| false).users_evaluated, 0);
    }

    #[test]
    fn per_user_split_evaluates_users_active_at_different_times() {
        // ten users, each in their own time range, each hears 1, 2, then 3
        let mut events = Vec::new();
        for user in 0..10 {
            let base = i64::from(user) * 10_000;
            events.push(event_of(user, 1, base));
            events.push(event_of(user, 2, base + 1));
            events.push(event_of(user, 3, base + 2));
        }

        let per_user = EvalConfig {
            test_fraction: 0.34,
            split: SplitMode::PerUser,
            ..EvalConfig::default()
        };
        let report = evaluate(&events, &per_user, &lenient_engine()).unwrap();
        assert_eq!(report.users_evaluated, 10);

        // one global cutoff only reaches the users active at the very end
        let by_time = EvalConfig {
            test_fraction: 0.34,
            split: SplitMode::Global,
            ..EvalConfig::default()
        };
        let report = evaluate(&events, &by_time, &lenient_engine()).unwrap();
        assert!(report.users_evaluated < 10);
    }

    #[test]
    fn invalid_config_is_rejected() {
        let engine = EngineConfig::default();
        let events = pattern_events();

        let bad_fraction = EvalConfig {
            test_fraction: 1.0,
            ..EvalConfig::default()
        };
        assert!(matches!(
            evaluate(&events, &bad_fraction, &engine),
            Err(EvalError::InvalidFraction(_))
        ));

        let zero_k = EvalConfig {
            k: 0,
            ..EvalConfig::default()
        };
        assert!(matches!(
            evaluate(&events, &zero_k, &engine),
            Err(EvalError::ZeroK)
        ));

        assert!(matches!(
            evaluate(&[], &EvalConfig::default(), &engine),
            Err(EvalError::NoEvents)
        ));
    }
}
