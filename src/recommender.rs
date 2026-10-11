use std::collections::HashMap;

use crate::models::candidate::{RecommendationContext, Source};
use crate::models::catalog::Catalog;
use crate::ranker::{Ranker, Reason, Recommendation};

#[derive(Debug, Clone)]
pub struct RecommenderConfig {
    /// The ranker is asked for this many times more tracks than needed,
    /// so the rules below have something to choose from.
    pub pool_factor: usize,
    /// Bonus for a track of the artist the user listens to most. Smaller shares get less.
    pub artist_weight: f64,
    /// No more than this many tracks of one artist in a list.
    pub max_per_artist: usize,
    /// How many popular tracks are kept for users without history.
    pub cold_start_pool: usize,
}

impl Default for RecommenderConfig {
    fn default() -> Self {
        RecommenderConfig {
            pool_factor: 5,
            artist_weight: 0.2,
            max_per_artist: 3,
            cold_start_pool: 100,
        }
    }
}

/// Model weights for one request. `None` keeps the configured weight.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeightOverrides {
    pub trends: Option<f64>,
    pub markov: Option<f64>,
    pub co_listening: Option<f64>,
    pub cf: Option<f64>,
}

impl WeightOverrides {
    fn apply(&self, source: Source, configured: f64) -> f64 {
        let chosen = match source {
            Source::Trends => self.trends,
            Source::Markov => self.markov,
            Source::CoListening => self.co_listening,
            Source::Cf => self.cf,
            Source::Artist | Source::Country | Source::Popular => None,
        };
        chosen.unwrap_or(configured)
    }
}

/// The four models plus the rules around them: cold start, favorite
/// artists and variety.
pub struct Recommender {
    ranker: Ranker,
    catalog: Catalog,
    config: RecommenderConfig,
}

impl Recommender {
    pub fn new(ranker: Ranker, catalog: Catalog, config: RecommenderConfig) -> Self {
        Recommender {
            ranker,
            catalog,
            config,
        }
    }

    /// `history` is oldest first. Without history the user gets what is popular.
    pub fn recommend(
        &self,
        history: Vec<u32>,
        country: Option<&str>,
        top: usize,
        overrides: WeightOverrides,
    ) -> Vec<Recommendation> {
        if history.is_empty() {
            return self.cold_start(country, top);
        }

        let affinity = self.artist_affinity(&history);
        let context = RecommendationContext { history };
        let pool = self.ranker.recommend_with(
            &context,
            top * self.config.pool_factor,
            |source, weight| overrides.apply(source, weight),
        );
        self.rerank(pool, &affinity, top)
    }

    /// Share of the user's plays by artist, 1.0 for the most played one.
    fn artist_affinity(&self, history: &[u32]) -> HashMap<u32, f64> {
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for &track in history {
            if let Some(artist) = self.catalog.artist_of(track) {
                *counts.entry(artist).or_default() += 1;
            }
        }
        let max = counts.values().copied().max().unwrap_or(0) as f64;
        counts
            .into_iter()
            .map(|(artist, count)| (artist, count as f64 / max))
            .collect()
    }

    fn rerank(
        &self,
        mut pool: Vec<Recommendation>,
        affinity: &HashMap<u32, f64>,
        top: usize,
    ) -> Vec<Recommendation> {
        for recommendation in &mut pool {
            let share = self
                .catalog
                .artist_of(recommendation.track_index)
                .and_then(|artist| affinity.get(&artist))
                .copied()
                .unwrap_or(0.0);
            let bonus = self.config.artist_weight * share;
            if bonus > 0.0 {
                recommendation.score += bonus;
                recommendation.reasons.push(Reason {
                    source: Source::Artist,
                    contribution: bonus,
                });
                recommendation
                    .reasons
                    .sort_by(|a, b| b.contribution.total_cmp(&a.contribution));
            }
        }
        pool.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.track_index.cmp(&b.track_index))
        });
        self.take_varied(pool, top)
    }

    fn cold_start(&self, country: Option<&str>, top: usize) -> Vec<Recommendation> {
        let local = country
            .map(|country| self.catalog.popular_in(country))
            .unwrap_or(&[]);
        let (popular, source) = if local.is_empty() {
            (self.catalog.popular_overall(), Source::Popular)
        } else {
            (local, Source::Country)
        };

        let best = popular.first().map_or(1.0, |&(_, plays)| plays as f64);
        let ranked = popular.iter().map(|&(track_index, plays)| {
            let score = plays as f64 / best;
            Recommendation {
                track_index,
                score,
                reasons: vec![Reason {
                    source,
                    contribution: score,
                }],
            }
        });
        self.take_varied(ranked, top)
    }

    /// The first `top` items, skipping tracks of an artist that already has enough.
    fn take_varied(
        &self,
        ranked: impl IntoIterator<Item = Recommendation>,
        top: usize,
    ) -> Vec<Recommendation> {
        let mut per_artist: HashMap<u32, usize> = HashMap::new();
        let mut picked = Vec::with_capacity(top);

        for recommendation in ranked {
            if picked.len() == top {
                break;
            }
            if let Some(artist) = self.catalog.artist_of(recommendation.track_index) {
                let taken = per_artist.entry(artist).or_default();
                if *taken >= self.config.max_per_artist {
                    continue;
                }
                *taken += 1;
            }
            picked.push(recommendation);
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, user};
    use crate::domain::{ListenEvent, Track, User};
    use crate::models::candidate::{Candidate, CandidateGenerator};
    use crate::ranker::RankerConfig;

    struct Fixed(Vec<(u32, f64)>);

    impl CandidateGenerator for Fixed {
        fn source(&self) -> Source {
            Source::Markov
        }

        fn candidates(&self, _: &RecommendationContext, _: usize) -> Vec<Candidate> {
            self.0
                .iter()
                .map(|&(track_index, score)| Candidate { track_index, score })
                .collect()
        }
    }

    fn by(track_index: u32, artist_index: u32) -> Track {
        Track {
            track_index,
            artist_index,
            track_name: None,
            artist_name: None,
        }
    }

    fn recommender(
        candidates: Vec<(u32, f64)>,
        tracks: &[Track],
        users: &[User],
        events: &[ListenEvent],
    ) -> Recommender {
        let ranker =
            Ranker::new(RankerConfig::default()).with_generator(Box::new(Fixed(candidates)), 1.0);
        let catalog = Catalog::build(tracks, users, events, 10);
        Recommender::new(ranker, catalog, RecommenderConfig::default())
    }

    /// Tracks 1-4 are by artist 1, tracks 5 and 6 by artist 2.
    fn with_artists() -> Recommender {
        let tracks = [by(1, 1), by(2, 1), by(3, 1), by(4, 1), by(5, 2), by(6, 2)];
        recommender(
            vec![(1, 1.0), (2, 0.9), (3, 0.8), (4, 0.7), (5, 0.1)],
            &tracks,
            &[],
            &[],
        )
    }

    fn tracks_of(list: &[Recommendation]) -> Vec<u32> {
        list.iter().map(|item| item.track_index).collect()
    }

    #[test]
    fn one_artist_does_not_fill_the_list() {
        let list = with_artists().recommend(vec![6], None, 5, WeightOverrides::default());
        assert_eq!(tracks_of(&list), vec![1, 2, 3, 5]);
    }

    #[test]
    fn a_favorite_artist_gets_a_bonus_with_a_reason() {
        let list = with_artists().recommend(vec![6], None, 5, WeightOverrides::default());
        let favorite = list.iter().find(|item| item.track_index == 5).unwrap();

        assert!((favorite.score - 0.3).abs() < 1e-9);
        assert!(favorite.reasons.iter().any(|reason| reason.source == Source::Artist));
    }

    #[test]
    fn weights_can_change_for_one_call() {
        let recommender = with_artists();
        let muted = WeightOverrides {
            markov: Some(0.0),
            ..WeightOverrides::default()
        };

        assert!(recommender.recommend(vec![6], None, 5, muted).is_empty());
        assert!(!recommender.recommend(vec![6], None, 5, WeightOverrides::default()).is_empty());
    }

    #[test]
    fn new_listener_gets_what_is_popular_in_their_country() {
        let users = [
            User {
                country: Some("UK".to_string()),
                ..user(1)
            },
            User {
                country: Some("DE".to_string()),
                ..user(3)
            },
        ];
        let events = [event_of(1, 10, 0), event_of(3, 11, 0), event_of(3, 12, 1)];
        let recommender = recommender(Vec::new(), &[], &users, &events);

        let local = recommender.recommend(Vec::new(), Some("DE"), 5, WeightOverrides::default());
        assert_eq!(tracks_of(&local), vec![11, 12]);
        assert_eq!(local[0].reasons[0].source, Source::Country);

        let unknown = recommender.recommend(Vec::new(), Some("FR"), 5, WeightOverrides::default());
        assert_eq!(unknown.len(), 3);
        assert_eq!(unknown[0].reasons[0].source, Source::Popular);
    }
}