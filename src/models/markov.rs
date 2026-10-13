use std::collections::HashMap;

use crate::domain::Session;
use crate::models::online::OnlineCounts;

/// A pair of tracks must have been followed this many times before the
/// second-order model trusts it; rarer pairs fall back to the last track only.
const MIN_PAIR_SUPPORT: u32 = 2;

/// Markov model: "which track usually follows this one" (first order) and,
/// optionally, "which track follows these two" (second order, R2).
pub struct MarkovModel {
    // from_track -> (to_track -> how many times it followed)
    transitions: HashMap<u32, HashMap<u32, u32>>,
    // (track before, track) -> (to_track -> times); only with order 2
    pairs: Option<HashMap<(u32, u32), HashMap<u32, u32>>>,
}

impl MarkovModel {
    /// A first-order model.
    pub fn train(sessions: &[Session]) -> Self {
        Self::train_with_order(sessions, 1)
    }

    /// `order` 2 also learns which track follows each pair of tracks.
    /// Anything above 2 is treated as 2.
    pub fn train_with_order(sessions: &[Session], order: usize) -> Self {
        let mut transitions: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
        let mut pairs: HashMap<(u32, u32), HashMap<u32, u32>> = HashMap::new();

        for session in sessions {
            // windows() only looks inside ONE session, so tracks from
            // different sessions are never linked
            for pair in session.tracks.windows(2) {
                *transitions
                    .entry(pair[0])
                    .or_default()
                    .entry(pair[1])
                    .or_default() += 1;
            }
            if order >= 2 {
                for triple in session.tracks.windows(3) {
                    *pairs
                        .entry((triple[0], triple[1]))
                        .or_default()
                        .entry(triple[2])
                        .or_default() += 1;
                }
            }
        }

        MarkovModel {
            transitions,
            pairs: (order >= 2).then_some(pairs),
        }
    }

    /// 1 or 2.
    pub fn order(&self) -> usize {
        if self.pairs.is_some() { 2 } else { 1 }
    }

    /// Number of distinct transitions kept, of both orders.
    pub fn transition_count(&self) -> usize {
        let first: usize = self.transitions.values().map(HashMap::len).sum();
        let second: usize = self
            .pairs
            .as_ref()
            .map_or(0, |pairs| pairs.values().map(HashMap::len).sum());
        first + second
    }

    /// Most likely next tracks after the end of `history`, as (track_index, probability). With order 2 the
    /// last two tracks decide, if that pair was followed often enough;
    /// otherwise the last track alone does. `online` adds the plays recorded
    /// since the training (R1).
    pub fn predict_after(
        &self,
        history: &[u32],
        limit: usize,
        online: Option<&OnlineCounts>,
    ) -> Vec<(u32, f64)> {
        let mut counts: HashMap<u32, u32> = HashMap::new();

        if let (Some(pairs), [.., before, last]) = (&self.pairs, history) {
            let pair = (*before, *last);
            if let Some(next) = pairs.get(&pair) {
                add(&mut counts, next);
            }
            if let Some(online) = online {
                online.add_next_after_pair(pair, &mut counts);
            }
            if counts.values().sum::<u32>() >= MIN_PAIR_SUPPORT {
                return ranked(counts, limit);
            }
            counts.clear();
        }

        let Some(&last) = history.last() else {
            return Vec::new();
        };
        if let Some(next) = self.transitions.get(&last) {
            add(&mut counts, next);
        }
        if let Some(online) = online {
            online.add_next_after(last, &mut counts);
        }
        ranked(counts, limit)
    }
}

fn add(into: &mut HashMap<u32, u32>, from: &HashMap<u32, u32>) {
    for (&track, &times) in from {
        *into.entry(track).or_default() += times;
    }
}

/// Counts as probabilities, highest first; ties are broken by track_index.
fn ranked(counts: HashMap<u32, u32>, limit: usize) -> Vec<(u32, f64)> {
    let total: u32 = counts.values().sum();
    if total == 0 {
        return Vec::new();
    }
    let mut ranked: Vec<(u32, f64)> = counts
        .into_iter()
        .map(|(track, count)| (track, f64::from(count) / f64::from(total)))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::session;

    #[test]
    fn probabilities_follow_transition_counts() {
        let model =
            MarkovModel::train(&[session(&[1, 2, 3]), session(&[1, 2, 4]), session(&[1, 3])]);
        let next = model.predict_after(&[1], 5, None);

        assert_eq!(next.len(), 2);
        assert_eq!(next[0].0, 2);
        assert!((next[0].1 - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(next[1].0, 3);
        assert!((next[1].1 - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn tracks_from_different_sessions_are_not_linked() {
        let model = MarkovModel::train(&[session(&[1, 2]), session(&[3, 4])]);

        assert_eq!(model.predict_after(&[1], 5, None), vec![(2, 1.0)]);
        // 2 is the last track of its session, so nothing follows it
        assert!(model.predict_after(&[2], 5, None).is_empty());
    }

    #[test]
    fn unknown_track_gives_empty_prediction() {
        let model = MarkovModel::train(&[session(&[1, 2])]);
        assert!(model.predict_after(&[99], 5, None).is_empty());
    }

    /// After 2 comes 3 when 1 was before it, and 4 when 9 was before it.
    fn context_sessions() -> Vec<Session> {
        vec![
            session(&[1, 2, 3]),
            session(&[1, 2, 3]),
            session(&[9, 2, 4]),
            session(&[9, 2, 4]),
            session(&[9, 2, 4]),
        ]
    }

    #[test]
    fn second_order_uses_the_track_before() {
        let model = MarkovModel::train_with_order(&context_sessions(), 2);
        assert_eq!(model.order(), 2);
        assert_eq!(model.predict_after(&[1, 2], 5, None), vec![(3, 1.0)]);
        assert_eq!(model.predict_after(&[9, 2], 5, None), vec![(4, 1.0)]);

        // first order mixes both contexts
        let first = MarkovModel::train(&context_sessions());
        assert_eq!(first.predict_after(&[1, 2], 5, None)[0].0, 4);
    }

    #[test]
    fn rare_or_unknown_pairs_fall_back_to_the_last_track() {
        let model = MarkovModel::train_with_order(&context_sessions(), 2);
        // (7, 2) was never seen: the answer comes from 2 alone
        let fallback = model.predict_after(&[7, 2], 5, None);
        assert_eq!(fallback[0], (4, 0.6));
        // a single track has no pair at all
        assert_eq!(model.predict_after(&[2], 5, None).len(), 2);
    }

    #[test]
    fn online_plays_change_the_prediction_without_training() {
        let model = MarkovModel::train(&[session(&[1, 2])]);
        let online = OnlineCounts::default();
        for _ in 0..3 {
            online.record(&[1], 5);
        }
        let next = model.predict_after(&[1], 5, Some(&online));
        assert_eq!(next[0], (5, 0.75));
        assert_eq!(next[1], (2, 0.25));
    }
}
