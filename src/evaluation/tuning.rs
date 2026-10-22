use crate::engine::Weights;

use super::{EvalReport, Evaluator};

/// Every combination of the five weights that are multiples of `1 / steps`
/// and add up to 1. With `steps = 10` that is 1001 combinations.
/// Only the ratio between weights matters, so this covers every possible mix.
pub fn weight_grid(steps: usize) -> Vec<Weights> {
    let share = |part: usize| part as f64 / steps as f64;

    let mut grid = Vec::new();
    for trends in 0..=steps {
        for markov in 0..=(steps - trends) {
            for co_listening in 0..=(steps - trends - markov) {
                for cf in 0..=(steps - trends - markov - co_listening) {
                    let mf = steps - trends - markov - co_listening - cf;
                    grid.push(Weights {
                        trends: share(trends),
                        markov: share(markov),
                        co_listening: share(co_listening),
                        cf: share(cf),
                        mf: share(mf),
                    });
                }
            }
        }
    }
    grid
}

pub struct Tuned {
    pub best: Weights,
    /// Best combinations on the tuning half, best first.
    pub top: Vec<(Weights, EvalReport)>,
    pub best_tuning: EvalReport,
    pub best_holdout: EvalReport,
    pub default_tuning: EvalReport,
    pub default_holdout: EvalReport,
}

/// Picks weights on one half of the users (even positions) and then checks
/// them on the other half (odd positions). The second number is the honest
/// one: weights picked and judged on the same users always look better than
/// they really are.
pub fn tune(evaluator: &mut Evaluator, steps: usize, default: Weights, top: usize) -> Tuned {
    let tuning_half = |position: usize| position.is_multiple_of(2);
    let holdout_half = |position: usize| !position.is_multiple_of(2);

    let mut results: Vec<(Weights, EvalReport)> = weight_grid(steps)
        .into_iter()
        .map(|weights| {
            evaluator.set_weights(weights);
            (weights, evaluator.run_where(tuning_half))
        })
        .collect();

    // best NDCG first; equal NDCG falls back to hit rate, then to grid order
    results.sort_by(|a, b| {
        b.1.ndcg
            .total_cmp(&a.1.ndcg)
            .then(b.1.hit_rate.total_cmp(&a.1.hit_rate))
    });

    let (best, best_tuning) = results[0].clone();
    evaluator.set_weights(best);
    let best_holdout = evaluator.run_where(holdout_half);

    evaluator.set_weights(default);
    let default_tuning = evaluator.run_where(tuning_half);
    let default_holdout = evaluator.run_where(holdout_half);

    results.truncate(top);
    Tuned {
        best,
        top: results,
        best_tuning,
        best_holdout,
        default_tuning,
        default_holdout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ListenEvent;
    use crate::domain::test_utils::event_of;
    use crate::engine::EngineConfig;
    use crate::evaluation::EvalConfig;
    use crate::evaluation::split::SplitMode;
    use crate::models::co_listening::CoListeningConfig;

    #[test]
    fn grid_has_expected_size_and_every_row_sums_to_one() {
        let grid = weight_grid(10);
        assert_eq!(grid.len(), 1001);
        for w in &grid {
            assert!((w.trends + w.markov + w.co_listening + w.cf + w.mf - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn grid_contains_single_model_corners() {
        let grid = weight_grid(4);
        assert!(grid.iter().any(|w| w.markov == 1.0 && w.trends == 0.0));
        assert!(
            grid.iter()
                .any(|w| w.co_listening == 1.0 && w.trends == 0.0)
        );
        assert!(grid.iter().any(|w| w.cf == 1.0 && w.markov == 0.0));
    }

    /// Many users who all follow 1 -> 2 -> 3, and later users who heard 1 and 2
    /// and then 3. Markov and co-listening predict this, trends cannot.
    fn events() -> Vec<ListenEvent> {
        let mut events = Vec::new();
        for user in 0..8 {
            let base = i64::from(user) * 100;
            events.push(event_of(user, 1, base));
            events.push(event_of(user, 2, base + 1));
            events.push(event_of(user, 3, base + 2));
        }
        for user in 8..16 {
            let base = 10_000 + i64::from(user) * 100;
            events.push(event_of(user, 1, base));
            events.push(event_of(user, 2, base + 1));
            events.push(event_of(user, 3, base + 2));
        }
        events
    }

    #[test]
    fn tuning_prefers_models_that_work_and_checks_on_other_users() {
        let eval = EvalConfig {
            k: 3,
            test_fraction: 0.34,
            split: SplitMode::PerUser,
            ..EvalConfig::default()
        };
        let engine = EngineConfig {
            co_listening: CoListeningConfig {
                min_cooccurrence: 1,
                min_jaccard: 0.0,
                ..CoListeningConfig::default()
            },
            ..EngineConfig::default()
        };
        let mut evaluator = Evaluator::new(&events(), &eval, &engine).unwrap();

        let tuned = tune(&mut evaluator, 4, engine.weights, 3);

        assert_eq!(tuned.top.len(), 3);
        // sorted from best to worst
        assert!(tuned.top[0].1.ndcg >= tuned.top[1].1.ndcg);
        // the two halves contain different users
        let total = evaluator.case_count();
        assert_eq!(
            tuned.best_tuning.users_evaluated + tuned.best_holdout.users_evaluated,
            total
        );
        // tuned weights are at least as good as the default on the tuning half
        assert!(tuned.best_tuning.ndcg >= tuned.default_tuning.ndcg);
    }
}
