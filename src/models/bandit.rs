//! Thompson sampling over the models (R3). Every like or skip of a track is
//! credited to the model that put the track on the list. For each listener
//! and model, the share of likes has a Beta(1 + likes, 1 + skips) belief; one
//! draw from each belief scales the model's weight for one request. Models
//! with many likes get more room, models with many skips less, and models
//! with little feedback are still tried now and then.

use std::collections::HashMap;

use crate::domain::SourceFeedback;
use crate::engine::Weights;
use crate::models::candidate::Source;
use crate::ranker::Reason;
use crate::recommender::WeightOverrides;
use crate::rng::Rng;

/// The five models with a weight are the arms of the bandit.
pub const ARMS: [Source; 5] = [
    Source::Trends,
    Source::Markov,
    Source::CoListening,
    Source::Cf,
    Source::Mf,
];

/// Likes and skips one listener gave to one model's tracks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArmStats {
    pub likes: u32,
    pub skips: u32,
}

impl ArmStats {
    /// The expected share of likes before any draw: `(1 + likes) / (2 + likes + skips)`.
    pub fn mean(self) -> f64 {
        (1.0 + f64::from(self.likes)) / (2.0 + f64::from(self.likes) + f64::from(self.skips))
    }
}

/// Is `source` one of the arms? Rules (artist, country, popular) are not.
pub fn is_arm(source: Source) -> bool {
    ARMS.contains(&source)
}

/// The model that did most to put a track on a list: the arm with the
/// largest contribution. Rules do not count, a track only they proposed has none.
pub fn credited_source(reasons: &[Reason]) -> Option<Source> {
    reasons
        .iter()
        .filter(|reason| is_arm(reason.source))
        .max_by(|a, b| a.contribution.total_cmp(&b.contribution))
        .map(|reason| reason.source)
}

/// Stored likes and skips per model name, as arm statistics.
pub fn arm_stats(rows: &[SourceFeedback]) -> HashMap<Source, ArmStats> {
    rows.iter()
        .filter_map(|row| {
            let source = Source::from_name(&row.source).filter(|source| is_arm(*source))?;
            Some((
                source,
                ArmStats {
                    likes: row.likes,
                    skips: row.skips,
                },
            ))
        })
        .collect()
}

fn weight_of(weights: Weights, source: Source) -> f64 {
    match source {
        Source::Trends => weights.trends,
        Source::Markov => weights.markov,
        Source::CoListening => weights.co_listening,
        Source::Cf => weights.cf,
        Source::Mf => weights.mf,
        Source::Artist | Source::Country | Source::Popular => 0.0,
    }
}

/// Weights for one request: each configured weight times `2 * draw`, so a
/// model with no feedback (draws average 0.5) keeps its weight on average.
/// Weights the request already set (`fixed`) are left alone.
pub fn sample_overrides(
    configured: Weights,
    stats: &HashMap<Source, ArmStats>,
    fixed: WeightOverrides,
    rng: &mut Rng,
) -> WeightOverrides {
    let mut draw = |source: Source, given: Option<f64>| {
        given.or_else(|| {
            let arm = stats.get(&source).copied().unwrap_or_default();
            let share = rng.beta(1.0 + f64::from(arm.likes), 1.0 + f64::from(arm.skips));
            Some(weight_of(configured, source) * 2.0 * share)
        })
    };
    WeightOverrides {
        trends: draw(Source::Trends, fixed.trends),
        markov: draw(Source::Markov, fixed.markov),
        co_listening: draw(Source::CoListening, fixed.co_listening),
        cf: draw(Source::Cf, fixed.cf),
        mf: draw(Source::Mf, fixed.mf),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured() -> Weights {
        Weights {
            trends: 0.25,
            markov: 0.25,
            co_listening: 0.25,
            cf: 0.25,
            mf: 0.25,
        }
    }

    #[test]
    fn liked_models_get_more_weight_on_average() {
        let stats = HashMap::from([
            (
                Source::Markov,
                ArmStats {
                    likes: 30,
                    skips: 2,
                },
            ),
            (
                Source::Cf,
                ArmStats {
                    likes: 1,
                    skips: 30,
                },
            ),
        ]);
        let mut rng = Rng::seeded(3);
        let (mut markov, mut cf, mut trends) = (0.0, 0.0, 0.0);
        let rounds = 2000;
        for _ in 0..rounds {
            let drawn =
                sample_overrides(configured(), &stats, WeightOverrides::default(), &mut rng);
            markov += drawn.markov.unwrap();
            cf += drawn.cf.unwrap();
            trends += drawn.trends.unwrap();
        }
        let rounds = f64::from(rounds);
        // 2 * 0.25 * mean: about 0.47 for markov, 0.03 for cf, 0.25 for trends
        assert!(
            (markov / rounds
                - 0.5
                    * ArmStats {
                        likes: 30,
                        skips: 2
                    }
                    .mean())
            .abs()
                < 0.02
        );
        assert!(cf / rounds < 0.05);
        assert!((trends / rounds - 0.25).abs() < 0.02);
    }

    #[test]
    fn weights_given_by_the_request_are_kept() {
        let fixed = WeightOverrides {
            markov: Some(0.0),
            ..WeightOverrides::default()
        };
        let drawn = sample_overrides(configured(), &HashMap::new(), fixed, &mut Rng::seeded(1));
        assert_eq!(drawn.markov, Some(0.0));
        assert!(drawn.cf.is_some());
    }

    #[test]
    fn the_strongest_model_gets_the_credit() {
        let reasons = [
            Reason {
                source: Source::Artist,
                contribution: 0.9,
            },
            Reason {
                source: Source::Cf,
                contribution: 0.2,
            },
            Reason {
                source: Source::Markov,
                contribution: 0.4,
            },
        ];
        assert_eq!(credited_source(&reasons), Some(Source::Markov));
        assert_eq!(credited_source(&reasons[..1]), None);

        let rows = [
            SourceFeedback {
                source: "markov".to_string(),
                likes: 2,
                skips: 1,
            },
            SourceFeedback {
                source: "artist".to_string(),
                likes: 5,
                skips: 0,
            },
        ];
        let stats = arm_stats(&rows);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[&Source::Markov], ArmStats { likes: 2, skips: 1 });
    }

    #[test]
    fn rules_are_not_arms() {
        assert!(is_arm(Source::Cf));
        assert!(!is_arm(Source::Artist));
        assert!((ArmStats::default().mean() - 0.5).abs() < 1e-9);
    }
}
