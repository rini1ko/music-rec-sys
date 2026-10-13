//! Quality beyond accuracy (R4). NDCG and hit rate reward guessing what the
//! user played next; these say how much of the catalog the lists use, how
//! unknown the tracks are and how many artists one list mixes.

use std::collections::{HashMap, HashSet};

/// Self-information of a track: `-log2(plays / all plays)`. A track nobody
/// played in training counts as played once, so it gets the highest value.
pub fn surprise(plays: usize, total_plays: usize) -> f64 {
    if total_plays == 0 {
        return 0.0;
    }
    let share = plays.max(1) as f64 / total_plays as f64;
    -share.log2()
}

/// Mean surprise of the tracks of one list; `None` for an empty list.
pub fn novelty(
    recommended: &[u32],
    popularity: &HashMap<u32, usize>,
    total_plays: usize,
) -> Option<f64> {
    if recommended.is_empty() {
        return None;
    }
    let sum: f64 = recommended
        .iter()
        .map(|track| surprise(popularity.get(track).copied().unwrap_or(0), total_plays))
        .sum();
    Some(sum / recommended.len() as f64)
}

/// Share of different artists in one list: 1.0 when every track is by
/// someone else. Tracks with an unknown artist count as their own artist.
pub fn artist_diversity(recommended: &[u32], artists: &HashMap<u32, u32>) -> Option<f64> {
    if recommended.is_empty() {
        return None;
    }
    let distinct: HashSet<(bool, u32)> = recommended
        .iter()
        .map(|track| match artists.get(track) {
            Some(&artist) => (true, artist),
            None => (false, *track),
        })
        .collect();
    Some(distinct.len() as f64 / recommended.len() as f64)
}

/// Collects the lists of every user and turns them into the three numbers.
#[derive(Debug, Default)]
pub struct BeyondAccuracy {
    shown: HashSet<u32>,
    novelty_sum: f64,
    novelty_lists: usize,
    diversity_sum: f64,
    diversity_lists: usize,
}

impl BeyondAccuracy {
    pub fn add(
        &mut self,
        recommended: &[u32],
        popularity: &HashMap<u32, usize>,
        total_plays: usize,
        artists: Option<&HashMap<u32, u32>>,
    ) {
        self.shown.extend(recommended.iter().copied());
        if let Some(value) = novelty(recommended, popularity, total_plays) {
            self.novelty_sum += value;
            self.novelty_lists += 1;
        }
        if let Some(value) = artists.and_then(|artists| artist_diversity(recommended, artists)) {
            self.diversity_sum += value;
            self.diversity_lists += 1;
        }
    }

    /// Share of the catalog that appeared in at least one list.
    pub fn coverage(&self, catalog_size: usize) -> f64 {
        if catalog_size == 0 {
            0.0
        } else {
            self.shown.len() as f64 / catalog_size as f64
        }
    }

    pub fn novelty(&self) -> f64 {
        mean(self.novelty_sum, self.novelty_lists)
    }

    /// `None` when the artists of the tracks are unknown.
    pub fn diversity(&self) -> Option<f64> {
        (self.diversity_lists > 0).then(|| mean(self.diversity_sum, self.diversity_lists))
    }
}

fn mean(sum: f64, count: usize) -> f64 {
    if count == 0 { 0.0 } else { sum / count as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rare_tracks_are_more_surprising() {
        // 1 of 8 plays is 3 bits, 4 of 8 is 1 bit
        assert!((surprise(1, 8) - 3.0).abs() < 1e-9);
        assert!((surprise(4, 8) - 1.0).abs() < 1e-9);
        // never played counts as once
        assert!((surprise(0, 8) - 3.0).abs() < 1e-9);
        assert_eq!(surprise(3, 0), 0.0);
    }

    #[test]
    fn novelty_is_the_mean_surprise_of_a_list() {
        let popularity = HashMap::from([(1, 4), (2, 1)]);
        assert!((novelty(&[1, 2], &popularity, 8).unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(novelty(&[], &popularity, 8), None);
    }

    #[test]
    fn diversity_counts_different_artists() {
        let artists = HashMap::from([(1, 10), (2, 10), (3, 20)]);
        assert!((artist_diversity(&[1, 2, 3], &artists).unwrap() - 2.0 / 3.0).abs() < 1e-9);
        // unknown artists are not merged with each other
        assert!((artist_diversity(&[7, 8], &artists).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn lists_add_up_to_coverage_novelty_and_diversity() {
        let popularity = HashMap::from([(1, 4), (2, 4)]);
        let artists = HashMap::from([(1, 10), (2, 20)]);
        let mut beyond = BeyondAccuracy::default();
        beyond.add(&[1, 2], &popularity, 8, Some(&artists));
        beyond.add(&[1], &popularity, 8, Some(&artists));

        assert!((beyond.coverage(4) - 0.5).abs() < 1e-9);
        assert!((beyond.novelty() - 1.0).abs() < 1e-9);
        assert_eq!(beyond.diversity(), Some(1.0));

        let without_artists = BeyondAccuracy::default();
        assert_eq!(without_artists.diversity(), None);
        assert_eq!(without_artists.coverage(0), 0.0);
    }
}
