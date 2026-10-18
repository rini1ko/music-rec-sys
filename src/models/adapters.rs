use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Duration;

use crate::domain::ListenEvent;
use crate::models::candidate::{Candidate, CandidateGenerator, RecommendationContext, Source};
use crate::models::cf::CfModel;
use crate::models::co_listening::CoListeningGraph;
use crate::models::markov::MarkovModel;
use crate::models::mf::MfModel;
use crate::models::online::OnlineCounts;
use crate::models::trends::trending;

/// Popular tracks. Ignores the user, so it also covers users without history.
pub struct TrendsGenerator {
    ranked: Vec<(u32, usize)>,
    online: Option<Arc<OnlineCounts>>,
}

impl TrendsGenerator {
    pub fn new(events: &[ListenEvent], window: Duration, pool_size: usize) -> Self {
        Self::from_ranked(trending(events, window, pool_size))
    }

    /// For a ranking that was already counted elsewhere, e.g. by the database.
    pub fn from_ranked(ranked: Vec<(u32, usize)>) -> Self {
        TrendsGenerator {
            ranked,
            online: None,
        }
    }

    /// Plays recorded after the training count too (R1).
    #[must_use]
    pub fn with_online(mut self, online: Arc<OnlineCounts>) -> Self {
        self.online = Some(online);
        self
    }

    /// The trained ranking plus the plays since the training, most played first.
    fn current(&self) -> Vec<(u32, usize)> {
        let Some(online) = self.online.as_ref().filter(|online| online.events() > 0) else {
            return self.ranked.clone();
        };
        let mut plays: HashMap<u32, usize> = self.ranked.iter().copied().collect();
        online.add_plays(&mut plays);
        let mut ranked: Vec<(u32, usize)> = plays.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
    }
}

impl CandidateGenerator for TrendsGenerator {
    fn source(&self) -> Source {
        Source::Trends
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        // heard tracks go before the cut, or they would take places the ranker
        // throws away afterwards
        let heard: HashSet<u32> = context.history.iter().copied().collect();
        self.current()
            .into_iter()
            .filter(|(track_index, _)| !heard.contains(track_index))
            .take(limit)
            .map(|(track_index, plays)| Candidate {
                track_index,
                score: plays as f64,
            })
            .collect()
    }

    fn size(&self) -> usize {
        self.ranked.len()
    }
}

/// What usually follows the last track (or the last two) the user listened to.
pub struct MarkovGenerator {
    model: MarkovModel,
    online: Option<Arc<OnlineCounts>>,
}

impl MarkovGenerator {
    pub fn new(model: MarkovModel) -> Self {
        MarkovGenerator {
            model,
            online: None,
        }
    }

    /// Transitions recorded after the training count too (R1).
    #[must_use]
    pub fn with_online(mut self, online: Arc<OnlineCounts>) -> Self {
        self.online = Some(online);
        self
    }
}

impl CandidateGenerator for MarkovGenerator {
    fn source(&self) -> Source {
        Source::Markov
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        // the model cuts its list at `limit`, so ask for enough to still have
        // `limit` tracks after the heard ones (often the likeliest) are removed
        let heard: HashSet<u32> = context.history.iter().copied().collect();
        let wanted = limit.saturating_add(heard.len());
        self.model
            .predict_after(&context.history, wanted, self.online.as_deref())
            .into_iter()
            .filter(|(track_index, _)| !heard.contains(track_index))
            .take(limit)
            .map(|(track_index, probability)| Candidate {
                track_index,
                score: probability,
            })
            .collect()
    }

    fn size(&self) -> usize {
        self.model.transition_count()
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
        let heard: HashSet<u32> = context.history.iter().copied().collect();
        let mut scores: HashMap<u32, f64> = HashMap::new();

        for &track in &context.history[start..] {
            // the recent tracks are each other's strongest neighbours, so ask
            // for enough to still have `limit` tracks without the heard ones
            let wanted = limit.saturating_add(heard.len());
            for (similar, similarity) in self.graph.similar(track, wanted) {
                if !heard.contains(&similar) {
                    *scores.entry(similar).or_default() += similarity;
                }
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

    fn size(&self) -> usize {
        self.graph.edge_count()
    }
}

/// Tracks whose hidden factors match the factors of the user's history (R8).
pub struct MfGenerator {
    model: MfModel,
}

impl MfGenerator {
    pub fn new(model: MfModel) -> Self {
        MfGenerator { model }
    }
}

impl CandidateGenerator for MfGenerator {
    fn source(&self) -> Source {
        Source::Mf
    }

    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate> {
        self.model
            .recommend(&context.history, limit)
            .into_iter()
            .map(|(track_index, score)| Candidate { track_index, score })
            .collect()
    }

    fn size(&self) -> usize {
        self.model.item_count()
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

    fn size(&self) -> usize {
        self.model.user_count()
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
        candidates
            .iter()
            .map(|c| (c.track_index, c.score))
            .collect()
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
        // 2 is heard, so only 1 is left
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].track_index, 1);
        assert!((candidates[0].score - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn heard_tracks_do_not_take_places_of_the_limit() {
        let tracks = |candidates: Vec<Candidate>| -> Vec<u32> {
            candidates.iter().map(|c| c.track_index).collect()
        };

        let co_listening = CoListeningGenerator::new(lenient_graph(), 5);
        assert_eq!(tracks(co_listening.candidates(&context(&[1, 2]), 1)), [3]);

        // after 2 the likeliest track is 1, which the listener already heard
        let sessions = [
            session(&[1, 2]),
            session(&[2, 1]),
            session(&[2, 1]),
            session(&[2, 3]),
        ];
        let markov = MarkovGenerator::new(MarkovModel::train(&sessions));
        assert_eq!(tracks(markov.candidates(&context(&[1, 2]), 1)), [3]);

        let events = [event(1, 0), event(1, 1), event(2, 2)];
        let trends = TrendsGenerator::new(&events, Duration::days(1), 10);
        assert_eq!(tracks(trends.candidates(&context(&[1]), 1)), [2]);
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
