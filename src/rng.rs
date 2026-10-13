//! A small random number generator for the bandit (R3) and the load
//! simulation (R6). Statistical quality is enough for sampling weights and
//! picking test data; it is not meant for anything secret.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static STREAM: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

/// SplitMix64: one `u64` of state, passes the usual statistical tests.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// The same seed gives the same numbers, which tests rely on.
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A different sequence on every call: the clock mixed with a counter, so
    /// two generators made in the same nanosecond still differ.
    pub fn from_time() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos() as u64);
        let stream = STREAM.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
        Self::seeded(nanos ^ stream)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`, from the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `0..bound`; `bound` must not be 0.
    pub fn below(&mut self, bound: usize) -> usize {
        // the modulo bias is far below anything a simulation could notice
        (self.next_u64() % bound as u64) as usize
    }

    /// Standard normal, by the Box-Muller transform.
    pub fn normal(&mut self) -> f64 {
        let u1 = 1.0 - self.next_f64(); // (0, 1], so ln never sees 0
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Gamma(shape, 1) by Marsaglia and Tsang. Shapes below 1 use the
    /// boost `Gamma(a) = Gamma(a + 1) * U^(1/a)`.
    pub fn gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            let boost = self.next_f64().max(f64::MIN_POSITIVE).powf(1.0 / shape);
            return self.gamma(shape + 1.0) * boost;
        }
        let d = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let x = self.normal();
            let v = (1.0 + c * x).powi(3);
            if v <= 0.0 {
                continue;
            }
            let u = self.next_f64();
            if u.ln() < 0.5 * x * x + d - d * v + d * v.ln() {
                return d * v;
            }
        }
    }

    /// Beta(a, b) as `X / (X + Y)` with `X ~ Gamma(a)`, `Y ~ Gamma(b)`.
    pub fn beta(&mut self, a: f64, b: f64) -> f64 {
        let x = self.gamma(a);
        let y = self.gamma(b);
        if x + y == 0.0 { 0.5 } else { x / (x + y) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean(samples: impl Iterator<Item = f64>) -> f64 {
        let values: Vec<f64> = samples.collect();
        values.iter().sum::<f64>() / values.len() as f64
    }

    #[test]
    fn the_same_seed_repeats_the_sequence() {
        let mut a = Rng::seeded(7);
        let mut b = Rng::seeded(7);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(Rng::from_time().next_u64(), Rng::from_time().next_u64());
    }

    #[test]
    fn uniform_values_stay_in_range() {
        let mut rng = Rng::seeded(1);
        for _ in 0..10_000 {
            let value = rng.next_f64();
            assert!((0.0..1.0).contains(&value));
            assert!(rng.below(3) < 3);
        }
    }

    #[test]
    fn beta_samples_have_the_expected_mean() {
        let mut rng = Rng::seeded(42);
        // Beta(a, b) has the mean a / (a + b)
        let likes = mean((0..20_000).map(|_| rng.beta(9.0, 3.0)));
        assert!((likes - 0.75).abs() < 0.01, "{likes}");
        let uniform = mean((0..20_000).map(|_| rng.beta(1.0, 1.0)));
        assert!((uniform - 0.5).abs() < 0.01, "{uniform}");
        let small = mean((0..20_000).map(|_| rng.beta(0.5, 0.5)));
        assert!((small - 0.5).abs() < 0.02, "{small}");
    }
}
