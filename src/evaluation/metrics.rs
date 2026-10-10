use std::collections::HashSet;

/// Share of the top-k recommendations that are relevant.
/// The denominator is always `k`, so a short list is not rewarded.
pub fn precision_at_k(recommended: &[u32], relevant: &HashSet<u32>, k: usize) -> f64 {
    if k == 0 {
        return 0.0;
    }
    hits(recommended, relevant, k) as f64 / k as f64
}

/// 1.0 if at least one recommendation in the top-k is relevant, else 0.0.
/// Averaged over users this is the share of users who got something useful.
pub fn hit_at_k(recommended: &[u32], relevant: &HashSet<u32>, k: usize) -> f64 {
    if hits(recommended, relevant, k) > 0 { 1.0 } else { 0.0 }
}

/// Share of the relevant tracks that made it into the top-k.
pub fn recall_at_k(recommended: &[u32], relevant: &HashSet<u32>, k: usize) -> f64 {
    if relevant.is_empty() {
        return 0.0;
    }
    hits(recommended, relevant, k) as f64 / relevant.len() as f64
}

/// Normalized discounted cumulative gain with binary relevance.
/// A hit at the top of the list is worth more than a hit at the bottom.
pub fn ndcg_at_k(recommended: &[u32], relevant: &HashSet<u32>, k: usize) -> f64 {
    let dcg: f64 = recommended
        .iter()
        .take(k)
        .enumerate()
        .filter(|(_, track)| relevant.contains(track))
        .map(|(position, _)| discount(position))
        .sum();

    // the best possible list puts every relevant track first
    let ideal: f64 = (0..relevant.len().min(k)).map(discount).sum();

    if ideal == 0.0 { 0.0 } else { dcg / ideal }
}

fn hits(recommended: &[u32], relevant: &HashSet<u32>, k: usize) -> usize {
    recommended
        .iter()
        .take(k)
        .filter(|track| relevant.contains(track))
        .count()
}

/// position 0 -> 1.0, position 1 -> 0.63, position 2 -> 0.5, ...
fn discount(position: usize) -> f64 {
    1.0 / ((position + 2) as f64).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[u32]) -> HashSet<u32> {
        items.iter().copied().collect()
    }

    #[test]
    fn precision_divides_by_k_even_for_short_lists() {
        assert!((precision_at_k(&[1, 2], &set(&[1]), 4) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn precision_ignores_everything_after_k() {
        assert_eq!(precision_at_k(&[9, 9, 1], &set(&[1]), 2), 0.0);
    }

    #[test]
    fn recall_divides_by_number_of_relevant_tracks() {
        assert!((recall_at_k(&[1, 5, 6], &set(&[1, 2, 3, 4]), 3) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn recall_without_relevant_tracks_is_zero() {
        assert_eq!(recall_at_k(&[1], &set(&[]), 3), 0.0);
    }

    #[test]
    fn perfect_ranking_has_ndcg_one() {
        assert!((ndcg_at_k(&[1, 2, 9], &set(&[1, 2]), 3) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn earlier_hit_gives_higher_ndcg() {
        let relevant = set(&[1]);
        assert!(ndcg_at_k(&[1, 9], &relevant, 2) > ndcg_at_k(&[9, 1], &relevant, 2));
    }

    #[test]
    fn ndcg_of_a_hit_at_second_position_is_known_value() {
        // 1 / log2(3) = 0.6309...
        let value = ndcg_at_k(&[9, 1], &set(&[1]), 2);
        assert!((value - 1.0 / 3f64.log2()).abs() < 1e-9);
    }

    #[test]
    fn no_hits_gives_zero_everywhere() {
        let relevant = set(&[1]);
        assert_eq!(precision_at_k(&[2, 3], &relevant, 2), 0.0);
        assert_eq!(recall_at_k(&[2, 3], &relevant, 2), 0.0);
        assert_eq!(ndcg_at_k(&[2, 3], &relevant, 2), 0.0);
    }

    #[test]
    fn k_zero_is_safe() {
        let relevant = set(&[1]);
        assert_eq!(precision_at_k(&[1], &relevant, 0), 0.0);
        assert_eq!(ndcg_at_k(&[1], &relevant, 0), 0.0);
    }
    
    #[test]
    fn hit_is_one_when_any_recommendation_is_relevant() {
        assert_eq!(hit_at_k(&[9, 1, 8], &set(&[1, 2]), 3), 1.0);
        assert_eq!(hit_at_k(&[9, 8, 1], &set(&[1]), 2), 0.0);
    }
}