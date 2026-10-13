pub mod beyond;
pub mod compare;
pub mod metrics;
mod rules;
pub mod split;
pub mod tuning;
pub mod windows;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use chrono::Duration;
use thiserror::Error;

use crate::domain::{ListenEvent, Session, Track, User};
use crate::engine::{EngineConfig, Weights, build_ranker_from_data};
use crate::etl::sessionizer::sessionize_all;
use crate::models::adapters::MarkovGenerator;
use crate::models::candidate::{RecommendationContext, Source};
use crate::models::catalog::Catalog;
use crate::models::markov::MarkovModel;
use crate::ranker::Ranker;
use crate::recommender::RecommenderConfig;

use beyond::BeyondAccuracy;

use metrics::{hit_at_k, ndcg_at_k, precision_at_k, recall_at_k};
use split::{Split, SplitMode, split};

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("test_fraction must be strictly between 0 and 1, got {0}")]
    InvalidFraction(f64),
    #[error("k must be at least 1")]
    ZeroK,
    #[error("there are no events to evaluate on")]
    NoEvents,
    #[error("there must be at least one time window")]
    ZeroWindows,
    #[error("a time window must be longer than zero")]
    EmptyWindow,
    #[error("the time windows reach back further than a date can go")]
    WindowsTooLong,
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
    /// Share of the training catalog shown to at least one user (R4).
    pub coverage: f64,
    /// Mean self-information of the shown tracks, in bits: higher is less known.
    pub novelty: f64,
    /// Mean share of different artists in one list; `None` without artist data.
    pub diversity: Option<f64>,
}

impl fmt::Display for EvalReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hit@{k} {:.4}  ndcg@{k} {:.4}  precision {:.4}  recall {:.4}  coverage {:.4}  novelty {:.2}",
            self.hit_rate,
            self.ndcg,
            self.precision,
            self.recall,
            self.coverage,
            self.novelty,
            k = self.k
        )?;
        if let Some(diversity) = self.diversity {
            write!(f, "  diversity {diversity:.3}")?;
        }
        write!(f, "  (users: {})", self.users_evaluated)
    }
}

/// One user to judge: what we know about them and what they listened to later.
struct Case {
    user: u32,
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
    /// Kept to rebuild the Markov model with another order (R2).
    train_sessions: Vec<Session>,
    markov_order: usize,
    /// Training plays per track, for novelty and coverage (R4).
    popularity: HashMap<u32, usize>,
    artists: Option<HashMap<u32, u32>>,
    /// Users without training history; only the rules (cold start) can serve them (E5).
    cold_cases: Vec<Case>,
    /// The training part, to build the catalog of the rules from.
    train_data: Vec<ListenEvent>,
    catalog: Option<Catalog>,
    countries: HashMap<u32, String>,
    rules: RecommenderConfig,
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
        Ok(Self::from_split(parts, eval.k, eval.session_gap, engine))
    }

    /// Trains on `parts.train` and prepares one case per user of `parts.test`.
    /// The time windows (W1) cut the events themselves and start here; `k`
    /// must be at least 1.
    pub fn from_split(
        parts: Split,
        k: usize,
        session_gap: Duration,
        engine: &EngineConfig,
    ) -> Self {
        // everything below uses ONLY training data, models never see the test part
        let train_sessions = sessionize_all(parts.train.clone(), session_gap);
        let ranker = build_ranker_from_data(&parts.train, &train_sessions, engine);
        let histories = histories(&parts.train);
        let mut popularity: HashMap<u32, usize> = HashMap::new();
        for event in &parts.train {
            *popularity.entry(event.track_index).or_default() += 1;
        }

        let mut evaluator = Evaluator {
            k,
            ranker,
            cases: Vec::new(),
            train_events: parts.train.len(),
            test_events: parts.test.len(),
            skipped_cold_start: 0,
            skipped_nothing_new: 0,
            train_sessions,
            markov_order: engine.markov_order,
            popularity,
            artists: None,
            cold_cases: Vec::new(),
            train_data: parts.train,
            catalog: None,
            countries: HashMap::new(),
            rules: engine.recommender.clone(),
        };

        for (user, test_tracks) in tracks_by_user(&parts.test) {
            let Some(history) = histories.get(&user) else {
                evaluator.skipped_cold_start += 1;
                evaluator.cold_cases.push(Case {
                    user,
                    history: Vec::new(),
                    relevant: test_tracks,
                });
                continue;
            };

            let heard: HashSet<u32> = history.iter().copied().collect();
            let relevant: HashSet<u32> = test_tracks.difference(&heard).copied().collect();
            if relevant.is_empty() {
                evaluator.skipped_nothing_new += 1;
                continue;
            }

            evaluator.cases.push(Case {
                user,
                history: history.clone(),
                relevant,
            });
        }

        evaluator
    }

    pub fn set_weights(&mut self, weights: Weights) {
        self.ranker.set_weight(Source::Trends, weights.trends);
        self.ranker.set_weight(Source::Markov, weights.markov);
        self.ranker
            .set_weight(Source::CoListening, weights.co_listening);
        self.ranker.set_weight(Source::Cf, weights.cf);
        self.ranker.set_weight(Source::Mf, weights.mf);
    }

    /// With the tracks and users the report also has the list diversity, and
    /// [`Evaluator::run_rules`] knows artists and countries (E5).
    #[must_use]
    pub fn with_catalog(mut self, tracks: &[Track], users: &[User]) -> Self {
        self.artists = Some(
            tracks
                .iter()
                .map(|track| (track.track_index, track.artist_index))
                .collect(),
        );
        self.catalog = Some(Catalog::build(
            tracks,
            users,
            &self.train_data,
            self.rules.cold_start_pool,
        ));
        self.countries = users
            .iter()
            .filter_map(|user| Some((user.user_index, user.country.clone()?)))
            .collect();
        self
    }

    /// Order of the Markov model in use (1 or 2).
    pub fn markov_order(&self) -> usize {
        self.markov_order
    }

    /// Retrains only the Markov model with another order, on the same
    /// training sessions (R2). Cheap compared to building the evaluator.
    pub fn set_markov_order(&mut self, order: usize) {
        if order == self.markov_order {
            return;
        }
        let model = MarkovModel::train_with_order(&self.train_sessions, order);
        self.ranker
            .replace_generator(Box::new(MarkovGenerator::new(model)));
        self.markov_order = model_order(order);
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
        let cases = self
            .cases
            .iter()
            .enumerate()
            .filter(|(position, _)| keep(*position))
            .map(|(_, case)| case);
        self.judge(cases, self.skipped_cold_start, |case| {
            let context = RecommendationContext {
                history: case.history.clone(),
            };
            self.ranker
                .recommend(&context, self.k)
                .into_iter()
                .map(|r| r.track_index)
                .collect()
        })
    }

    /// Averages the metrics of `rank` over `cases`.
    fn judge<'a>(
        &self,
        cases: impl Iterator<Item = &'a Case>,
        skipped_cold_start: usize,
        rank: impl Fn(&Case) -> Vec<u32>,
    ) -> EvalReport {
        let mut report = EvalReport {
            k: self.k,
            train_events: self.train_events,
            test_events: self.test_events,
            users_evaluated: 0,
            skipped_cold_start,
            skipped_nothing_new: self.skipped_nothing_new,
            hit_rate: 0.0,
            precision: 0.0,
            recall: 0.0,
            ndcg: 0.0,
            coverage: 0.0,
            novelty: 0.0,
            diversity: None,
        };
        let mut beyond = BeyondAccuracy::default();

        for case in cases {
            let recommended = rank(case);

            report.users_evaluated += 1;
            report.hit_rate += hit_at_k(&recommended, &case.relevant, self.k);
            report.precision += precision_at_k(&recommended, &case.relevant, self.k);
            report.recall += recall_at_k(&recommended, &case.relevant, self.k);
            report.ndcg += ndcg_at_k(&recommended, &case.relevant, self.k);
            beyond.add(
                &recommended,
                &self.popularity,
                self.train_events,
                self.artists.as_ref(),
            );
        }
        report.coverage = beyond.coverage(self.popularity.len());
        report.novelty = beyond.novelty();
        report.diversity = beyond.diversity();

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

/// Orders above 2 are trained as 2, see [`MarkovModel::train_with_order`].
fn model_order(order: usize) -> usize {
    order.clamp(1, 2)
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
    fn beyond_accuracy_metrics_are_filled_in() {
        let tracks: Vec<crate::domain::Track> = (1..=3)
            .map(|index| crate::domain::test_utils::named_track(index, index, "t", "a"))
            .collect();
        let evaluator =
            Evaluator::new(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine())
                .unwrap()
                .with_catalog(&tracks, &[]);
        let report = evaluator.run();

        // only track 3 is new to both users, and it is one of three known tracks
        assert!((report.coverage - 1.0 / 3.0).abs() < 1e-9);
        assert!(report.novelty > 0.0);
        assert_eq!(report.diversity, Some(1.0));
    }

    #[test]
    fn the_markov_order_can_change_without_rebuilding() {
        let mut evaluator =
            Evaluator::new(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine()).unwrap();
        assert_eq!(evaluator.markov_order(), 1);
        evaluator.set_weights(Weights {
            markov: 1.0,
            ..Weights::ZERO
        });
        let first = evaluator.run();

        evaluator.set_markov_order(2);
        assert_eq!(evaluator.markov_order(), 2);
        // (1, 2) -> 3 was seen four times, so order 2 predicts it as well
        let second = evaluator.run();
        assert!((second.hit_rate - first.hit_rate).abs() < 1e-9);
        evaluator.set_markov_order(5);
        assert_eq!(evaluator.markov_order(), 2);
    }

    #[test]
    fn every_model_gets_a_row_and_the_hybrid_comes_back() {
        let mut evaluator =
            Evaluator::new(&pattern_events(), &global(3, 2.0 / 18.0), &lenient_engine()).unwrap();
        let hybrid = lenient_engine().weights;
        let rows = compare::compare_models(&mut evaluator, hybrid);
        let names: Vec<&str> = rows.iter().map(|row| row.model).collect();
        assert_eq!(
            names,
            [
                "trends",
                "markov",
                "markov_2",
                "co_listening",
                "cf",
                "mf",
                "hybrid",
                "hybrid_rules"
            ]
        );
        assert_eq!(evaluator.markov_order(), 1);
        assert_eq!(evaluator.run(), rows[6].report);
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
        let markov_only = EngineConfig {
            weights: Weights {
                markov: 1.0,
                ..Weights::ZERO
            },
            ..lenient_engine()
        };
        let nothing = EngineConfig {
            weights: Weights::ZERO,
            ..lenient_engine()
        };

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
