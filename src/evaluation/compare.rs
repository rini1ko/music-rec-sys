//! Each model on its own against the hybrid, on the same users. Shared by the
//! `eval` command and by `POST /model/evaluate`.

use crate::engine::Weights;
use crate::evaluation::{EvalReport, Evaluator};

/// One row of the comparison: which weights were used and how they did.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelScore {
    /// `trends`, `markov`, `markov_2`, `co_listening`, `cf`, `mf`, `hybrid` or
    /// `hybrid_rules` (the hybrid with cold start, artist bonus and variety).
    pub model: &'static str,
    pub report: EvalReport,
}

/// One preset: its name, the weights and the Markov order (`None` keeps the configured one).
pub type Preset = (&'static str, Weights, Option<usize>);

/// One model at a time, the Markov model in both orders (R2), and the hybrid.
#[must_use]
pub fn model_presets(hybrid: Weights) -> [Preset; 7] {
    let markov = Weights {
        markov: 1.0,
        ..Weights::ZERO
    };
    [
        (
            "trends",
            Weights {
                trends: 1.0,
                ..Weights::ZERO
            },
            None,
        ),
        ("markov", markov, Some(1)),
        ("markov_2", markov, Some(2)),
        (
            "co_listening",
            Weights {
                co_listening: 1.0,
                ..Weights::ZERO
            },
            None,
        ),
        (
            "cf",
            Weights {
                cf: 1.0,
                ..Weights::ZERO
            },
            None,
        ),
        (
            "mf",
            Weights {
                mf: 1.0,
                ..Weights::ZERO
            },
            None,
        ),
        ("hybrid", hybrid, None),
    ]
}

/// Runs every preset. The evaluator gets its Markov order and the hybrid
/// weights back afterwards.
pub fn compare_models(evaluator: &mut Evaluator, hybrid: Weights) -> Vec<ModelScore> {
    let configured_order = evaluator.markov_order();
    let scores = model_presets(hybrid)
        .into_iter()
        .map(|(model, weights, order)| {
            evaluator.set_markov_order(order.unwrap_or(configured_order));
            evaluator.set_weights(weights);
            ModelScore {
                model,
                report: evaluator.run(),
            }
        })
        .collect();
    evaluator.set_markov_order(configured_order);
    evaluator.set_weights(hybrid);
    // the hybrid once more, now with the rules around it (E5)
    let mut scores: Vec<ModelScore> = scores;
    scores.push(ModelScore {
        model: "hybrid_rules",
        report: evaluator.run_rules(),
    });
    scores
}
