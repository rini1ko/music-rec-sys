use std::collections::HashMap;

use crate::domain::Session;

/// First-order Markov model: "which track usually follows this one".
pub struct MarkovModel {
    // from_track -> (to_track -> how many times it followed)
    transitions: HashMap<u32, HashMap<u32, u32>>,
}

impl MarkovModel {
    pub fn train(sessions: &[Session]) -> Self {
        let mut transitions: HashMap<u32, HashMap<u32, u32>> = HashMap::new();

        for session in sessions {
            // windows(2) gives every pair of neighbours inside ONE session,
            // so tracks from different sessions are never linked
            for pair in session.tracks.windows(2) {
                *transitions
                    .entry(pair[0])
                    .or_default()
                    .entry(pair[1])
                    .or_default() += 1;
            }
        }

        MarkovModel { transitions }
    }

    /// Most likely next tracks as (track_index, probability).
    pub fn predict(&self, track_index: u32, limit: usize) -> Vec<(u32, f64)> {
        let Some(next) = self.transitions.get(&track_index) else {
            return Vec::new();
        };

        let total: u32 = next.values().sum();
        let mut ranked: Vec<(u32, f64)> = next
            .iter()
            .map(|(&track, &count)| (track, count as f64 / total as f64))
            .collect();

        // highest probability first; ties are broken by track_index
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(limit);
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::session;

    #[test]
    fn probabilities_follow_transition_counts() {
        let model =
            MarkovModel::train(&[session(&[1, 2, 3]), session(&[1, 2, 4]), session(&[1, 3])]);
        let next = model.predict(1, 5);

        assert_eq!(next.len(), 2);
        assert_eq!(next[0].0, 2);
        assert!((next[0].1 - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(next[1].0, 3);
        assert!((next[1].1 - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn tracks_from_different_sessions_are_not_linked() {
        let model = MarkovModel::train(&[session(&[1, 2]), session(&[3, 4])]);

        assert_eq!(model.predict(1, 5), vec![(2, 1.0)]);
        // 2 is the last track of its session, so nothing follows it
        assert!(model.predict(2, 5).is_empty());
    }

    #[test]
    fn unknown_track_gives_empty_prediction() {
        let model = MarkovModel::train(&[session(&[1, 2])]);
        assert!(model.predict(99, 5).is_empty());
    }
}
