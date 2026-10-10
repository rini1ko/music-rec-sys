use std::collections::{BTreeMap, HashMap, HashSet};

use crate::domain::ListenEvent;

#[derive(Debug, Clone, Copy)]
pub struct CfConfig {
    /// How many of the most similar users are used for one recommendation.
    pub neighbors: usize,
    /// A user must share at least this many tracks to count as similar.
    pub min_overlap: usize,
    /// From every neighbor only the most played tracks are proposed.
    pub favourites_per_neighbor: usize,
}

impl Default for CfConfig {
    fn default() -> Self {
        CfConfig {
            neighbors: 50,
            min_overlap: 3,
            favourites_per_neighbor: 200,
        }
    }
}

/// User-based collaborative filtering ("people with similar taste listened to this").
///
/// Every user is the set of tracks they listened to. The user we recommend for
/// is compared with all of them by cosine similarity of these sets. The most
/// similar users ("neighbors") vote for their most played tracks, and a vote
/// counts more when the neighbor is more similar.
///
/// The model works from a list of tracks and not from a user id, so it also
/// works for a user who was not in the training data.
pub struct CfModel {
    config: CfConfig,
    /// Number of different tracks of every user, the position is the user's slot.
    profile_sizes: Vec<usize>,
    /// track -> slots of the users who listened to it. With it only users who
    /// share at least one track are looked at, not all of them.
    listeners: HashMap<u32, Vec<usize>>,
    /// slot -> most played tracks of that user, each with a weight in (0, 1].
    favourites: Vec<Vec<(u32, f64)>>,
}

impl CfModel {
    pub fn train(events: &[ListenEvent], config: &CfConfig) -> Self {
        // user -> track -> number of plays; BTreeMap keeps the order stable
        let mut plays: BTreeMap<u32, BTreeMap<u32, u32>> = BTreeMap::new();
        for event in events {
            *plays
                .entry(event.user_index)
                .or_default()
                .entry(event.track_index)
                .or_default() += 1;
        }

        let mut profile_sizes = Vec::with_capacity(plays.len());
        let mut favourites = Vec::with_capacity(plays.len());
        let mut listeners: HashMap<u32, Vec<usize>> = HashMap::new();

        for (slot, (_, tracks)) in plays.into_iter().enumerate() {
            profile_sizes.push(tracks.len());
            for &track in tracks.keys() {
                listeners.entry(track).or_default().push(slot);
            }

            let mut ranked: Vec<(u32, u32)> = tracks.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            ranked.truncate(config.favourites_per_neighbor);

            // log keeps one track played 500 times from drowning everything else
            let max_plays = ranked.first().map_or(1, |&(_, plays)| plays);
            let norm = f64::from(max_plays).ln_1p();
            favourites.push(
                ranked
                    .into_iter()
                    .map(|(track, plays)| (track, f64::from(plays).ln_1p() / norm))
                    .collect(),
            );
        }

        CfModel {
            config: *config,
            profile_sizes,
            listeners,
            favourites,
        }
    }

    /// Number of users the model was trained on.
    pub fn user_count(&self) -> usize {
        self.profile_sizes.len()
    }

    /// Tracks the neighbors listened to and the user did not, best first.
    pub fn recommend(&self, history: &[u32], limit: usize) -> Vec<(u32, f64)> {
        let heard: HashSet<u32> = history.iter().copied().collect();
        if heard.is_empty() || limit == 0 {
            return Vec::new();
        }

        // how many tracks every other user shares with this one
        let mut overlap: HashMap<usize, usize> = HashMap::new();
        for track in &heard {
            if let Some(slots) = self.listeners.get(track) {
                for &slot in slots {
                    *overlap.entry(slot).or_default() += 1;
                }
            }
        }

        let mut neighbors: Vec<(usize, f64)> = overlap
            .into_iter()
            // a user whose tracks are all heard already (this user's own
            // profile, for example) has nothing new to offer
            .filter(|&(slot, shared)| {
                shared >= self.config.min_overlap && shared < self.profile_sizes[slot]
            })
            .map(|(slot, shared)| {
                let norm = ((heard.len() * self.profile_sizes[slot]) as f64).sqrt();
                (slot, shared as f64 / norm)
            })
            .collect();

        // most similar first; ties are broken by slot so the result is stable
        neighbors.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        neighbors.truncate(self.config.neighbors);

        let mut scores: HashMap<u32, f64> = HashMap::new();
        for (slot, similarity) in neighbors {
            for &(track, weight) in &self.favourites[slot] {
                if !heard.contains(&track) {
                    *scores.entry(track).or_default() += similarity * weight;
                }
            }
        }

        let mut ranked: Vec<(u32, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(limit);
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event_of;

    fn lenient() -> CfConfig {
        CfConfig {
            min_overlap: 1,
            ..CfConfig::default()
        }
    }

    /// One listen of every track, in the given order.
    fn listens(user: u32, tracks: &[u32]) -> Vec<ListenEvent> {
        tracks
            .iter()
            .enumerate()
            .map(|(i, &track)| event_of(user, track, i as i64))
            .collect()
    }

    fn all(groups: Vec<Vec<ListenEvent>>) -> Vec<ListenEvent> {
        groups.into_iter().flatten().collect()
    }

    #[test]
    fn recommends_tracks_of_similar_users_and_ignores_unrelated_ones() {
        let events = all(vec![
            listens(0, &[1, 2, 3]),
            listens(1, &[1, 2, 4]),
            listens(2, &[7, 8, 9]),
        ]);
        let model = CfModel::train(&events, &lenient());

        let recommended = model.recommend(&[1, 2], 10);

        let tracks: Vec<u32> = recommended.iter().map(|&(t, _)| t).collect();
        assert_eq!(tracks, vec![3, 4]);
        // cosine of {1,2} and {1,2,3}: 2 / sqrt(2 * 3)
        assert!((recommended[0].1 - 2.0 / 6f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn more_similar_neighbor_counts_more() {
        let events = all(vec![
            listens(0, &[1, 2, 10]),      // shares 2 of 3 tracks
            listens(1, &[1, 11, 12, 13]), // shares 1 of 4 tracks
        ]);
        let model = CfModel::train(&events, &lenient());

        let recommended = model.recommend(&[1, 2], 10);

        assert_eq!(recommended[0].0, 10);
        assert!(recommended[0].1 > recommended[1].1);
    }

    #[test]
    fn heavily_played_tracks_of_a_neighbor_come_first() {
        let mut events = listens(0, &[1]);
        for minute in 10..14 {
            events.push(event_of(0, 3, minute)); // track 3: 4 plays
        }
        events.push(event_of(0, 4, 20)); // track 4: 1 play
        let model = CfModel::train(&events, &lenient());

        let recommended = model.recommend(&[1], 10);

        assert_eq!(recommended[0].0, 3);
        assert_eq!(recommended[1].0, 4);
        // play weight of track 4: ln(2) / ln(5)
        let expected = recommended[0].1 * (2f64.ln() / 5f64.ln());
        assert!((recommended[1].1 - expected).abs() < 1e-9);
    }

    #[test]
    fn user_with_nothing_new_to_offer_is_skipped() {
        // the only user in the model has exactly the tracks of the history
        let model = CfModel::train(&listens(0, &[1, 2, 3]), &lenient());
        assert!(model.recommend(&[1, 2, 3], 10).is_empty());
    }

    #[test]
    fn too_little_overlap_is_not_enough() {
        let events = listens(0, &[1, 5, 6]);
        let strict = CfConfig {
            min_overlap: 2,
            ..CfConfig::default()
        };

        assert!(CfModel::train(&events, &strict).recommend(&[1, 2], 10).is_empty());
        assert!(!CfModel::train(&events, &lenient()).recommend(&[1, 2], 10).is_empty());
    }

    #[test]
    fn only_the_closest_neighbors_are_used() {
        let events = all(vec![listens(0, &[1, 2, 10]), listens(1, &[1, 11, 12, 13])]);
        let config = CfConfig {
            neighbors: 1,
            ..lenient()
        };
        let model = CfModel::train(&events, &config);

        let tracks: Vec<u32> = model.recommend(&[1, 2], 10).iter().map(|&(t, _)| t).collect();
        assert_eq!(tracks, vec![10]);
    }

    #[test]
    fn only_the_favourites_of_a_neighbor_are_proposed() {
        let mut events = listens(0, &[1, 4]);
        events.push(event_of(0, 3, 10));
        events.push(event_of(0, 3, 11)); // track 3 is the most played
        let config = CfConfig {
            favourites_per_neighbor: 1,
            ..lenient()
        };
        let model = CfModel::train(&events, &config);

        let tracks: Vec<u32> = model.recommend(&[1], 10).iter().map(|&(t, _)| t).collect();
        assert_eq!(tracks, vec![3]);
    }

    #[test]
    fn empty_history_and_zero_limit_give_nothing() {
        let model = CfModel::train(&listens(0, &[1, 2]), &lenient());
        assert!(model.recommend(&[], 10).is_empty());
        assert!(model.recommend(&[1], 0).is_empty());
    }

    #[test]
    fn limit_is_respected_and_heard_tracks_are_never_returned() {
        let model = CfModel::train(&listens(0, &[1, 2, 3, 4, 5]), &lenient());

        let recommended = model.recommend(&[1, 2], 2);

        assert_eq!(recommended.len(), 2);
        assert!(recommended.iter().all(|&(track, _)| track > 2));
    }

    #[test]
    fn counts_users() {
        let events = all(vec![listens(0, &[1]), listens(5, &[1]), listens(9, &[2])]);
        assert_eq!(CfModel::train(&events, &lenient()).user_count(), 3);
    }
}