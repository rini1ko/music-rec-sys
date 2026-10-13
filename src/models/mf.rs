//! Matrix factorization for implicit feedback (R8), after Hu, Koren and
//! Volinsky (2008). Every play says "this listener likes this track" with a
//! confidence that grows with the number of plays. Alternating least squares
//! learns a short vector per track so that a listener's vector, dotted with a
//! track's, predicts the preference.
//!
//! Listener vectors are not kept: one is solved from the history at request
//! time ("fold-in"), with the same step the training uses. So the model
//! serves new listeners and new plays without retraining.

use std::collections::HashMap;

use crate::domain::ListenEvent;
use crate::rng::Rng;

#[derive(Debug, Clone, PartialEq)]
pub struct MfConfig {
    /// Length of every vector.
    pub factors: usize,
    /// Rounds of "solve the listeners, then solve the tracks".
    pub iterations: usize,
    /// Ridge term that keeps the vectors small.
    pub regularization: f64,
    /// Confidence of `n` plays is `1 + alpha * ln(1 + n)`.
    pub alpha: f64,
    /// Tracks with fewer plays get no vector: too little to learn from.
    pub min_item_plays: usize,
    pub seed: u64,
}

impl Default for MfConfig {
    fn default() -> Self {
        Self {
            factors: 16,
            iterations: 6,
            regularization: 0.1,
            alpha: 2.0,
            min_item_plays: 2,
            seed: 7,
        }
    }
}

pub struct MfModel {
    k: usize,
    regularization: f64,
    alpha: f64,
    /// Track of each row of `item_factors`.
    items: Vec<u32>,
    index: HashMap<u32, usize>,
    /// Row-major, `items.len() x k`.
    item_factors: Vec<f64>,
    /// `Yᵀ Y` of the final track vectors, `k x k`; every fold-in starts from it.
    gram: Vec<f64>,
}

/// Plays of one listener (or one track) as (row of the other side, plays).
type Row = Vec<(usize, u32)>;

impl MfModel {
    pub fn train(events: &[ListenEvent], config: &MfConfig) -> Self {
        let k = config.factors.max(1);
        let mut per_track: HashMap<u32, usize> = HashMap::new();
        for event in events {
            *per_track.entry(event.track_index).or_default() += 1;
        }
        let mut items: Vec<u32> = per_track
            .into_iter()
            .filter(|&(_, plays)| plays >= config.min_item_plays)
            .map(|(track, _)| track)
            .collect();
        items.sort_unstable();
        let index: HashMap<u32, usize> = items
            .iter()
            .enumerate()
            .map(|(row, &track)| (track, row))
            .collect();

        let (user_rows, item_rows) = play_matrix(events, &index, items.len());
        let mut rng = Rng::seeded(config.seed);
        let mut item_factors: Vec<f64> = (0..items.len() * k).map(|_| rng.normal() * 0.1).collect();
        let mut user_factors = vec![0.0; user_rows.len() * k];

        let solve_side = |rows: &[Row], other: &[f64], into: &mut [f64]| {
            let gram = gram_of(other, k);
            for (row, observed) in rows.iter().enumerate() {
                let vector = solve_factor(
                    &gram,
                    observed,
                    other,
                    k,
                    config.regularization,
                    config.alpha,
                );
                into[row * k..(row + 1) * k].copy_from_slice(&vector);
            }
        };
        for _ in 0..config.iterations {
            solve_side(&user_rows, &item_factors, &mut user_factors);
            solve_side(&item_rows, &user_factors, &mut item_factors);
        }

        let gram = gram_of(&item_factors, k);
        MfModel {
            k,
            regularization: config.regularization,
            alpha: config.alpha,
            items,
            index,
            item_factors,
            gram,
        }
    }

    /// Tracks that have a vector.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// The vector of a listener with this history, `None` when no track of it has a vector.
    pub fn fold_in(&self, history: &[u32]) -> Option<Vec<f64>> {
        let mut plays: HashMap<usize, u32> = HashMap::new();
        for track in history {
            if let Some(&row) = self.index.get(track) {
                *plays.entry(row).or_default() += 1;
            }
        }
        if plays.is_empty() {
            return None;
        }
        let observed: Row = plays.into_iter().collect();
        Some(solve_factor(
            &self.gram,
            &observed,
            &self.item_factors,
            self.k,
            self.regularization,
            self.alpha,
        ))
    }

    /// Tracks with the highest predicted preference, unheard ones only, as
    /// (track_index, score); only positive scores.
    pub fn recommend(&self, history: &[u32], limit: usize) -> Vec<(u32, f64)> {
        let Some(user) = self.fold_in(history) else {
            return Vec::new();
        };
        let heard: std::collections::HashSet<u32> = history.iter().copied().collect();
        let mut scored: Vec<(u32, f64)> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, track)| !heard.contains(track))
            .map(|(row, &track)| {
                (
                    track,
                    dot(&user, &self.item_factors[row * self.k..(row + 1) * self.k]),
                )
            })
            .filter(|&(_, score)| score > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(limit);
        scored
    }
}

/// Listener rows and track rows of the play counts; tracks without a vector are left out.
fn play_matrix(
    events: &[ListenEvent],
    index: &HashMap<u32, usize>,
    items: usize,
) -> (Vec<Row>, Vec<Row>) {
    let mut counts: HashMap<(u32, usize), u32> = HashMap::new();
    for event in events {
        if let Some(&row) = index.get(&event.track_index) {
            *counts.entry((event.user_index, row)).or_default() += 1;
        }
    }
    let mut users: Vec<u32> = counts.keys().map(|&(user, _)| user).collect();
    users.sort_unstable();
    users.dedup();
    let user_row: HashMap<u32, usize> = users
        .iter()
        .enumerate()
        .map(|(row, &user)| (user, row))
        .collect();

    let mut user_rows: Vec<Row> = vec![Vec::new(); users.len()];
    let mut item_rows: Vec<Row> = vec![Vec::new(); items];
    let mut entries: Vec<_> = counts.into_iter().collect();
    // a fixed order keeps the training repeatable
    entries.sort_unstable();
    for ((user, item), plays) in entries {
        let user = user_row[&user];
        user_rows[user].push((item, plays));
        item_rows[item].push((user, plays));
    }
    (user_rows, item_rows)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// `Fᵀ F` of row-major factors with `k` columns.
fn gram_of(factors: &[f64], k: usize) -> Vec<f64> {
    let mut gram = vec![0.0; k * k];
    for row in factors.chunks_exact(k) {
        for i in 0..k {
            for j in 0..=i {
                gram[i * k + j] += row[i] * row[j];
            }
        }
    }
    for i in 0..k {
        for j in 0..i {
            gram[j * k + i] = gram[i * k + j];
        }
    }
    gram
}

/// One ALS step for one listener or track: with `Y` the other side,
/// `(YᵀY + Yᵀ(C - I)Y + λI) x = Yᵀ C p`. Only the observed rows differ from
/// `YᵀY`, which is why it is passed in ready.
fn solve_factor(
    gram: &[f64],
    observed: &[(usize, u32)],
    other: &[f64],
    k: usize,
    lambda: f64,
    alpha: f64,
) -> Vec<f64> {
    let mut a = gram.to_vec();
    let mut b = vec![0.0; k];
    for i in 0..k {
        a[i * k + i] += lambda;
    }
    for &(row, plays) in observed {
        let confidence = 1.0 + alpha * f64::from(plays).ln_1p();
        let y = &other[row * k..(row + 1) * k];
        for i in 0..k {
            b[i] += confidence * y[i];
            for j in 0..k {
                a[i * k + j] += (confidence - 1.0) * y[i] * y[j];
            }
        }
    }
    // the λ on the diagonal keeps the matrix positive definite
    solve_spd(&mut a, &mut b, k).unwrap_or_else(|| vec![0.0; k])
}

/// Solves `A x = b` for a symmetric positive definite `A` (`k x k`) by
/// Cholesky; `A` and `b` are used as scratch space. `None` if `A` is not
/// positive definite.
pub fn solve_spd(a: &mut [f64], b: &mut [f64], k: usize) -> Option<Vec<f64>> {
    // A = L Lᵀ, L stored in the lower triangle of `a`
    for j in 0..k {
        let mut diagonal = a[j * k + j];
        for p in 0..j {
            diagonal -= a[j * k + p] * a[j * k + p];
        }
        if diagonal <= 0.0 || !diagonal.is_finite() {
            return None;
        }
        let pivot = diagonal.sqrt();
        a[j * k + j] = pivot;
        for i in (j + 1)..k {
            let mut value = a[i * k + j];
            for p in 0..j {
                value -= a[i * k + p] * a[j * k + p];
            }
            a[i * k + j] = value / pivot;
        }
    }
    // L y = b, then Lᵀ x = y
    for i in 0..k {
        for p in 0..i {
            b[i] -= a[i * k + p] * b[p];
        }
        b[i] /= a[i * k + i];
    }
    for i in (0..k).rev() {
        for p in (i + 1)..k {
            b[i] -= a[p * k + i] * b[p];
        }
        b[i] /= a[i * k + i];
    }
    Some(b.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event_of;

    #[test]
    fn cholesky_solves_a_small_system() {
        // [4 2; 2 3] x = [2; 5] has x = [-0.5, 2]
        let mut a = vec![4.0, 2.0, 2.0, 3.0];
        let mut b = vec![2.0, 5.0];
        let x = solve_spd(&mut a, &mut b, 2).unwrap();
        assert!((x[0] + 0.5).abs() < 1e-12 && (x[1] - 2.0).abs() < 1e-12);

        let mut negative = vec![-1.0];
        assert!(solve_spd(&mut negative, &mut [1.0], 1).is_none());
    }

    /// Listeners 0-3 play tracks 1, 2 and 3; listeners 4-7 play 7, 8 and 9.
    fn two_tastes() -> Vec<ListenEvent> {
        let mut events = Vec::new();
        for user in 0..8u32 {
            let tracks = if user < 4 { [1, 2, 3] } else { [7, 8, 9] };
            for (minute, track) in tracks.into_iter().enumerate() {
                events.push(event_of(user, track, i64::from(user) * 10 + minute as i64));
            }
        }
        events
    }

    #[test]
    fn a_listener_gets_the_missing_track_of_their_taste() {
        let model = MfModel::train(&two_tastes(), &MfConfig::default());
        assert_eq!(model.item_count(), 6);

        let list = model.recommend(&[1, 2], 5);
        assert_eq!(list.first().map(|&(track, _)| track), Some(3));
        // the other taste is far below, if it is there at all
        assert!(list.iter().all(|&(track, _)| track == 3 || track >= 7));
        assert!(list.iter().skip(1).all(|&(_, score)| score < list[0].1));
    }

    #[test]
    fn rare_tracks_and_unknown_histories_give_nothing() {
        let config = MfConfig {
            min_item_plays: 5,
            ..MfConfig::default()
        };
        let model = MfModel::train(&two_tastes(), &config);
        assert_eq!(model.item_count(), 0);
        assert!(model.recommend(&[1], 5).is_empty());

        let model = MfModel::train(&two_tastes(), &MfConfig::default());
        assert!(model.fold_in(&[42]).is_none());
        assert!(model.recommend(&[], 5).is_empty());
    }

    #[test]
    fn training_is_repeatable() {
        let a = MfModel::train(&two_tastes(), &MfConfig::default());
        let b = MfModel::train(&two_tastes(), &MfConfig::default());
        assert_eq!(a.recommend(&[7], 3), b.recommend(&[7], 3));
    }
}
