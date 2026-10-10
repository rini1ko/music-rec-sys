use std::collections::HashMap;

use chrono::Duration;

use crate::domain::ListenEvent;
use crate::models::candidate::{Candidate, CandidateGenerator, RecommendationContext, Source};
use crate::models::co_listening::CoListeningGraph;
use crate::models::markov::MarkovModel;
use crate::models::trends::trending;
use crate::models::cf::CfModel;

/// Popular tracks. Ignores the user, so it also covers users without history.
pub struct TrendsGenerator {
    ranked: Vec<(u32, usize)>,
}

impl TrendsGenerator {
    pub fn new(events: &[ListenEvent], window: Duration, pool_size: usize) -> Self {
        TrendsGenerator {
            ranked: trending(events, window, pool_size),
        }
    }
}

impl CandidateGenerator for TrendsGenerator {
    fn source(&self) -> Source {
        Source::Trends
    }

    fn candidates(&self, _context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        self.ranked
            .iter()
            .take(limit)
            .map(|&(track_index, plays)| Candidate {
                track_index,
                score: plays as f64,
            })
            .collect()
    }
}

/// What usually follows the last track the user listened to.
pub struct MarkovGenerator {
    model: MarkovModel,
}

impl MarkovGenerator {
    pub fn new(model: MarkovModel) -> Self {
        MarkovGenerator { model }
    }
}

impl CandidateGenerator for MarkovGenerator {
    fn source(&self) -> Source {
        Source::Markov
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        let Some(&last_track) = context.history.last() else {
            return Vec::new();
        };

        self.model
            .predict(last_track, limit)
            .into_iter()
            .map(|(track_index, probability)| Candidate {
                track_index,
                score: probability,
            })
            .collect()
    }
}

/// Tracks similar to the user's most recent tracks.
pub struct CoListeningGenerator {
    graph: CoListeningGraph,
    recent_tracks: usize,
}

impl CoListeningGenerator {
    pub fn new(graph: CoListeningGraph, recent_tracks: usize) -> Self {
        CoListeningGenerator {
            graph,
            recent_tracks,
        }
    }
}

impl CandidateGenerator for CoListeningGenerator {
    fn source(&self) -> Source {
        Source::CoListening
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        let start = context.history.len().saturating_sub(self.recent_tracks);
        let mut scores: HashMap<u32, f64> = HashMap::new();

        for &track in &context.history[start..] {
            for (similar, similarity) in self.graph.similar(track, limit) {
                *scores.entry(similar).or_default() += similarity;
            }
        }

        let mut candidates: Vec<Candidate> = scores
            .into_iter()
            .map(|(track_index, score)| Candidate { track_index, score })
            .collect();

        // highest score first; ties are broken by track_index
        candidates.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.track_index.cmp(&b.track_index))
        });
        candidates.truncate(limit);
        candidates
    }
}

/// Tracks that users with a similar listening history liked.
pub struct CfGenerator {
    model: CfModel,
}

impl CfGenerator {
    pub fn new(model: CfModel) -> Self {
        CfGenerator { model }
    }
}

impl CandidateGenerator for CfGenerator {
    fn source(&self) -> Source {
        Source::Cf
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        self.model
            .recommend(&context.history, limit)
            .into_iter()
            .map(|(track_index, score)| Candidate { track_index, score })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event, session};
    use crate::models::co_listening::CoListeningConfig;

    fn context(history: &[u32]) -> RecommendationContext {
        RecommendationContext {
            history: history.to_vec(),
        }
    }

    fn pairs(candidates: &[Candidate]) -> Vec<(u32, f64)> {
        candidates.iter().map(|c| (c.track_index, c.score)).collect()
    }

    fn lenient_graph() -> CoListeningGraph {
        let config = CoListeningConfig {
            min_cooccurrence: 1,
            min_jaccard: 0.0,
            max_session_tracks: 100,
        };
        let sessions = [session(&[1, 2, 3]), session(&[1, 2]), session(&[1, 3])];
        CoListeningGraph::build(&sessions, &config)
    }

    #[test]
    fn trends_generator_ignores_context() {
        let events = [event(1, 0), event(1, 1), event(2, 2)];
        let generator = TrendsGenerator::new(&events, Duration::days(1), 10);

        let with_history = generator.candidates(&context(&[7, 8]), 10);
        let without_history = generator.candidates(&context(&[]), 10);

        assert_eq!(pairs(&with_history), vec![(1, 2.0), (2, 1.0)]);
        assert_eq!(pairs(&without_history), pairs(&with_history));
    }

    #[test]
    fn markov_generator_uses_last_track_of_history() {
        let model = MarkovModel::train(&[session(&[1, 2]), session(&[1, 2]), session(&[1, 3])]);
        let generator = MarkovGenerator::new(model);

        let candidates = generator.candidates(&context(&[5, 1]), 10);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].track_index, 2);
        assert!((candidates[0].score - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(candidates[1].track_index, 3);
    }

    #[test]
    fn markov_generator_gives_nothing_for_empty_history() {
        let generator = MarkovGenerator::new(MarkovModel::train(&[session(&[1, 2])]));
        assert!(generator.candidates(&context(&[]), 10).is_empty());
    }

    #[test]
    fn co_listening_generator_sums_similarity_over_recent_tracks() {
        let generator = CoListeningGenerator::new(lenient_graph(), 5);

        let candidates = generator.candidates(&context(&[2, 3]), 10);

        // track 1: 2/3 from track 2 plus 2/3 from track 3
        assert_eq!(candidates[0].track_index, 1);
        assert!((candidates[0].score - 4.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn co_listening_generator_only_looks_at_recent_tracks() {
        let generator = CoListeningGenerator::new(lenient_graph(), 1);

        let candidates = generator.candidates(&context(&[2, 3]), 10);

        // only track 3 is used: 1 is 2/3 similar, 2 is 1/3 similar
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].track_index, 1);
        assert!((candidates[0].score - 2.0 / 3.0).abs() < 1e-9);
    }
    
    #[test]
    fn cf_generator_proposes_tracks_of_similar_users() {
        use crate::domain::test_utils::event_of;
        use crate::models::cf::CfConfig;

        let events = [event_of(0, 1, 0), event_of(0, 2, 1), event_of(0, 3, 2)];
        let config = CfConfig {
            min_overlap: 1,
            ..CfConfig::default()
        };
        let generator = CfGenerator::new(CfModel::train(&events, &config));

        let candidates = generator.candidates(&context(&[1]), 10);

        assert_eq!(generator.source(), Source::Cf);
        let tracks: Vec<u32> = candidates.iter().map(|c| c.track_index).collect();
        assert_eq!(tracks, vec![2, 3]);
        assert!(generator.candidates(&context(&[]), 10).is_empty());
    }
}