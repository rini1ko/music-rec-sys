use std::collections::{HashMap, HashSet};

use crate::models::candidate::{CandidateGenerator, RecommendationContext, Source};

pub struct RankerConfig {
    /// How many candidates to request from each model before merging.
    pub candidates_per_source: usize,
}

impl Default for RankerConfig {
    fn default() -> Self {
        RankerConfig {
            candidates_per_source: 50,
        }
    }
}

/// One model's vote for a track: how much it added to the final score.
#[derive(Debug, Clone)]
pub struct Reason {
    pub source: Source,
    pub contribution: f64,
}

#[derive(Debug, Clone)]
pub struct Recommendation {
    pub track_index: u32,
    pub score: f64,
    /// Strongest reason first.
    pub reasons: Vec<Reason>,
}

impl Recommendation {
    pub fn explain(&self) -> String {
        self.reasons
            .iter()
            .map(|reason| reason.source.reason())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

struct WeightedGenerator {
    generator: Box<dyn CandidateGenerator>,
    weight: f64,
}

pub struct Ranker {
    generators: Vec<WeightedGenerator>,
    config: RankerConfig,
}

impl Ranker {
    pub fn new(config: RankerConfig) -> Self {
        Ranker {
            generators: Vec::new(),
            config,
        }
    }

    pub fn with_generator(mut self, generator: Box<dyn CandidateGenerator>, weight: f64) -> Self {
        self.generators.push(WeightedGenerator { generator, weight });
        self
    }

    /// Changes the weight of one model without retraining it.
    /// Weight tuning relies on this to try many combinations quickly.
    pub fn set_weight(&mut self, source: Source, weight: f64) {
        for entry in &mut self.generators {
            if entry.generator.source() == source {
                entry.weight = weight;
            }
        }
    }

    pub fn recommend(&self, context: &RecommendationContext, limit: usize) -> Vec<Recommendation> {
        let heard: HashSet<u32> = context.history.iter().copied().collect();
        let mut merged: HashMap<u32, Recommendation> = HashMap::new();

        for entry in &self.generators {
            if !entry.weight.is_finite() || entry.weight <= 0.0 {
                continue;
            }

            let mut candidates = entry
                .generator
                .candidates(context, self.config.candidates_per_source);
            // heard tracks are removed BEFORE normalizing, otherwise the best
            // remaining candidate of a model would never reach a score of 1.0
            candidates.retain(|c| {
                c.score.is_finite() && c.score > 0.0 && !heard.contains(&c.track_index)
            });

            let Some(max_score) = candidates.iter().map(|c| c.score).max_by(f64::total_cmp)
            else {
                continue;
            };

            for candidate in candidates {
                let contribution = entry.weight * candidate.score / max_score;
                let recommendation =
                    merged
                        .entry(candidate.track_index)
                        .or_insert_with(|| Recommendation {
                            track_index: candidate.track_index,
                            score: 0.0,
                            reasons: Vec::new(),
                        });
                recommendation.score += contribution;
                recommendation.reasons.push(Reason {
                    source: entry.generator.source(),
                    contribution,
                });
            }
        }

        let mut ranked: Vec<Recommendation> = merged.into_values().collect();
        ranked.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.track_index.cmp(&b.track_index))
        });
        ranked.truncate(limit);

        for recommendation in &mut ranked {
            recommendation
                .reasons
                .sort_by(|a, b| b.contribution.total_cmp(&a.contribution));
        }
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::candidate::Candidate;

    struct FakeGenerator {
        source: Source,
        items: Vec<(u32, f64)>,
    }

    impl FakeGenerator {
        fn boxed(source: Source, items: &[(u32, f64)]) -> Box<dyn CandidateGenerator> {
            Box::new(FakeGenerator {
                source,
                items: items.to_vec(),
            })
        }
    }

    impl CandidateGenerator for FakeGenerator {
        fn source(&self) -> Source {
            self.source
        }

        fn candidates(&self, _context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
            self.items
                .iter()
                .take(limit)
                .map(|&(track_index, score)| Candidate { track_index, score })
                .collect()
        }
    }

    fn context(history: &[u32]) -> RecommendationContext {
        RecommendationContext {
            history: history.to_vec(),
        }
    }

    fn two_source_ranker(markov_weight: f64) -> Ranker {
        Ranker::new(RankerConfig::default())
            .with_generator(
                FakeGenerator::boxed(Source::Markov, &[(1, 1.0), (2, 0.5)]),
                markov_weight,
            )
            .with_generator(
                FakeGenerator::boxed(Source::Trends, &[(2, 10.0), (3, 5.0)]),
                1.0,
            )
    }

    fn track_order(recommendations: &[Recommendation]) -> Vec<u32> {
        recommendations.iter().map(|r| r.track_index).collect()
    }

    #[test]
    fn scores_from_different_sources_are_added() {
        let recommendations = two_source_ranker(1.0).recommend(&context(&[]), 10);

        // track 1: 1.0, track 2: 0.5 + 1.0, track 3: 0.5
        assert_eq!(track_order(&recommendations), vec![2, 1, 3]);
        assert!((recommendations[0].score - 1.5).abs() < 1e-9);
        assert_eq!(recommendations[0].reasons.len(), 2);
        assert_eq!(recommendations[0].reasons[0].source, Source::Trends);
    }

    #[test]
    fn weights_change_the_order() {
        let recommendations = two_source_ranker(3.0).recommend(&context(&[]), 10);

        // track 1: 3.0, track 2: 1.5 + 1.0, track 3: 0.5
        assert_eq!(track_order(&recommendations), vec![1, 2, 3]);
        assert!((recommendations[1].score - 2.5).abs() < 1e-9);
    }

    #[test]
    fn heard_tracks_are_skipped_before_normalization() {
        let recommendations = two_source_ranker(1.0).recommend(&context(&[1]), 10);

        assert_eq!(track_order(&recommendations), vec![2, 3]);
        // without track 1, Markov's best candidate is track 2 and gets a full 1.0
        assert!((recommendations[0].score - 2.0).abs() < 1e-9);
    }

    #[test]
    fn no_generators_gives_no_recommendations() {
        let ranker = Ranker::new(RankerConfig::default());
        assert!(ranker.recommend(&context(&[1, 2]), 10).is_empty());
    }

    #[test]
    fn limit_truncates_the_result() {
        let recommendations = two_source_ranker(1.0).recommend(&context(&[]), 2);
        assert_eq!(track_order(&recommendations), vec![2, 1]);
    }

    #[test]
    fn non_positive_scores_are_ignored() {
        let ranker = Ranker::new(RankerConfig::default()).with_generator(
            FakeGenerator::boxed(Source::Trends, &[(1, 0.0), (2, -1.0), (3, 2.0)]),
            1.0,
        );
        let recommendations = ranker.recommend(&context(&[]), 10);

        assert_eq!(track_order(&recommendations), vec![3]);
        assert!((recommendations[0].score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn explanation_lists_reasons_in_order() {
        let recommendation = Recommendation {
            track_index: 1,
            score: 1.0,
            reasons: vec![
                Reason {
                    source: Source::Trends,
                    contribution: 0.6,
                },
                Reason {
                    source: Source::Markov,
                    contribution: 0.4,
                },
            ],
        };

        assert_eq!(
            recommendation.explain(),
            "popular right now; often played right after your last track"
        );
    }
    
    #[test]
    fn set_weight_changes_the_result_without_rebuilding() {
        let mut ranker = Ranker::new(RankerConfig::default())
            .with_generator(FakeGenerator::boxed(Source::Trends, &[(1, 1.0)]), 0.0);
        let context = RecommendationContext { history: vec![] };

        assert!(ranker.recommend(&context, 5).is_empty());

        ranker.set_weight(Source::Trends, 1.0);
        assert_eq!(ranker.recommend(&context, 5).len(), 1);

        // a weight for a model that is not in the ranker changes nothing
        ranker.set_weight(Source::Markov, 5.0);
        assert!((ranker.recommend(&context, 5)[0].score - 1.0).abs() < 1e-9);
    }
}