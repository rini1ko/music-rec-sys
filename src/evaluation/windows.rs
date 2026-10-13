//! Evaluation over consecutive time windows (W1). One train/test split shows
//! how the models did once. Moving the cutoff forward window by window shows
//! whether a model is good every week or only on the last one, and whether
//! it gets better or worse as the data grows.
//!
//! For every window `(from, to]` the models train on all events up to `from`
//! and are judged on the events inside the window, the same way a deployed
//! recommender would have been retrained at `from` and used until `to`.

use chrono::{Duration, NaiveDateTime};

use crate::domain::{ListenEvent, Track, User};
use crate::engine::EngineConfig;
use crate::evaluation::compare::{ModelScore, compare_models};
use crate::evaluation::split::Split;
use crate::evaluation::{EvalError, Evaluator};

/// NDCG differences below this count as a tie when the best model of a window is picked.
const TIE: f64 = 1e-9;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// How many windows; the newest one ends with the newest event.
    pub windows: usize,
    /// Length of one window.
    pub length: Duration,
    /// Length of the recommendation list that is judged.
    pub k: usize,
    /// Pause that splits the training events into sessions.
    pub session_gap: Duration,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            windows: 4,
            length: Duration::days(7),
            k: 10,
            session_gap: Duration::minutes(30),
        }
    }
}

/// The scores of every model in one window.
#[derive(Debug, Clone)]
pub struct WindowScores {
    /// The window starts right after this moment; training ends at it.
    pub from: NaiveDateTime,
    /// The window ends at this moment, inclusive.
    pub to: NaiveDateTime,
    pub train_events: usize,
    pub test_events: usize,
    /// Users the plain ranking judged in this window.
    pub users: usize,
    /// Each model alone, the hybrid and the hybrid with rules. Empty when the
    /// window had no training events, no plays or nobody to judge.
    pub scores: Vec<ModelScore>,
}

impl WindowScores {
    /// True when the window produced scores.
    pub fn is_judged(&self) -> bool {
        !self.scores.is_empty()
    }

    /// The models with the highest NDCG of this window; none when all scored zero.
    pub fn best(&self) -> Vec<&'static str> {
        let top = self
            .scores
            .iter()
            .map(|score| score.report.ndcg)
            .fold(0.0, f64::max);
        if top <= TIE {
            return Vec::new();
        }
        self.scores
            .iter()
            .filter(|score| top - score.report.ndcg <= TIE)
            .map(|score| score.model)
            .collect()
    }
}

/// How one model did across all the judged windows.
#[derive(Debug, Clone, PartialEq)]
pub struct Stability {
    pub model: &'static str,
    /// Windows the model was judged in.
    pub windows: usize,
    pub mean_ndcg: f64,
    /// Standard deviation of the NDCG over the windows: lower is steadier.
    pub std_ndcg: f64,
    pub min_ndcg: f64,
    pub max_ndcg: f64,
    pub mean_hit_rate: f64,
    /// Change of the NDCG per window along a least-squares line. Below zero
    /// the model gets worse in the newer windows.
    pub trend: f64,
    /// Windows where this model had the highest NDCG (ties count for each).
    pub wins: usize,
}

/// All the windows, oldest first, and the summary per model.
#[derive(Debug, Clone)]
pub struct Backtest {
    pub windows: Vec<WindowScores>,
    pub stability: Vec<Stability>,
}

/// The `(from, to]` bounds of the windows, oldest first. The newest window
/// ends at `latest`, each one starts where the previous one ends. The caller
/// checks that the whole span fits, see [`backtest`].
fn window_bounds(
    latest: NaiveDateTime,
    windows: usize,
    length: Duration,
) -> Vec<(NaiveDateTime, NaiveDateTime)> {
    (0..windows)
        .rev()
        .map(|back| {
            let to = latest - length * back as i32;
            (to - length, to)
        })
        .collect()
}

/// Trains and judges every model once per window. `catalog` adds the artist
/// diversity and lets the rules know artists and countries, like
/// [`Evaluator::with_catalog`].
pub fn backtest(
    events: &[ListenEvent],
    catalog: Option<(&[Track], &[User])>,
    config: &WindowConfig,
    engine: &EngineConfig,
) -> Result<Backtest, EvalError> {
    if config.k == 0 {
        return Err(EvalError::ZeroK);
    }
    if config.windows == 0 {
        return Err(EvalError::ZeroWindows);
    }
    if config.length <= Duration::zero() {
        return Err(EvalError::EmptyWindow);
    }
    let Some(latest) = events.iter().map(|event| event.timestamp).max() else {
        return Err(EvalError::NoEvents);
    };
    let span = i32::try_from(config.windows)
        .ok()
        .and_then(|windows| config.length.checked_mul(windows))
        .and_then(|span| latest.checked_sub_signed(span));
    if span.is_none() {
        return Err(EvalError::WindowsTooLong);
    }

    let windows: Vec<WindowScores> = window_bounds(latest, config.windows, config.length)
        .into_iter()
        .map(|(from, to)| judge_window(events, from, to, catalog, config, engine))
        .collect();
    let stability = summarize(&windows);
    Ok(Backtest { windows, stability })
}

fn judge_window(
    events: &[ListenEvent],
    from: NaiveDateTime,
    to: NaiveDateTime,
    catalog: Option<(&[Track], &[User])>,
    config: &WindowConfig,
    engine: &EngineConfig,
) -> WindowScores {
    let train: Vec<ListenEvent> = events
        .iter()
        .filter(|event| event.timestamp <= from)
        .cloned()
        .collect();
    let test: Vec<ListenEvent> = events
        .iter()
        .filter(|event| event.timestamp > from && event.timestamp <= to)
        .cloned()
        .collect();
    let mut window = WindowScores {
        from,
        to,
        train_events: train.len(),
        test_events: test.len(),
        users: 0,
        scores: Vec::new(),
    };
    if train.is_empty() || test.is_empty() {
        return window;
    }

    let parts = Split { train, test };
    let mut evaluator = Evaluator::from_split(parts, config.k, config.session_gap, engine);
    if let Some((tracks, users)) = catalog {
        evaluator = evaluator.with_catalog(tracks, users);
    }
    window.users = evaluator.case_count();
    if window.users == 0 && evaluator.cold_case_count() == 0 {
        return window;
    }
    window.scores = compare_models(&mut evaluator, engine.weights);
    window
}

/// One [`Stability`] row per model, in the order of [`compare_models`].
fn summarize(windows: &[WindowScores]) -> Vec<Stability> {
    let Some(first) = windows.iter().find(|window| window.is_judged()) else {
        return Vec::new();
    };
    let bests: Vec<Vec<&'static str>> = windows.iter().map(WindowScores::best).collect();

    first
        .scores
        .iter()
        .map(|score| score.model)
        .map(|model| {
            // (window position, ndcg, hit rate) of every window that judged the model
            let points: Vec<(f64, f64, f64)> = windows
                .iter()
                .enumerate()
                .filter_map(|(position, window)| {
                    let score = window.scores.iter().find(|score| score.model == model)?;
                    Some((position as f64, score.report.ndcg, score.report.hit_rate))
                })
                .collect();
            let ndcg: Vec<f64> = points.iter().map(|point| point.1).collect();
            let line: Vec<(f64, f64)> = points.iter().map(|point| (point.0, point.1)).collect();
            let hit_rates: Vec<f64> = points.iter().map(|point| point.2).collect();

            Stability {
                model,
                windows: points.len(),
                mean_ndcg: mean(&ndcg),
                std_ndcg: std_dev(&ndcg),
                min_ndcg: ndcg.iter().copied().fold(f64::INFINITY, f64::min),
                max_ndcg: ndcg.iter().copied().fold(0.0, f64::max),
                mean_hit_rate: mean(&hit_rates),
                trend: slope(&line),
                wins: bests.iter().filter(|best| best.contains(&model)).count(),
            }
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Population standard deviation; 0 for fewer than two values.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let center = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - center).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

/// Slope of the least-squares line through `(x, y)`; 0 when it is not defined.
fn slope(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let n = points.len() as f64;
    let mean_x = points.iter().map(|point| point.0).sum::<f64>() / n;
    let mean_y = points.iter().map(|point| point.1).sum::<f64>() / n;
    let spread: f64 = points.iter().map(|point| (point.0 - mean_x).powi(2)).sum();
    if spread == 0.0 {
        return 0.0;
    }
    let together: f64 = points
        .iter()
        .map(|point| (point.0 - mean_x) * (point.1 - mean_y))
        .sum();
    together / spread
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event_of;

    /// Ten users walk through a ring of twelve tracks, three a day: user `u`
    /// plays `u + 3d`, `u + 3d + 1`, `u + 3d + 2` on day `d`. Each day brings
    /// tracks the user has not heard but other users have, in the same order.
    fn daily_album(days: i64) -> Vec<ListenEvent> {
        let mut events = Vec::new();
        for day in 0..days {
            for user in 0..10u32 {
                let start = day * 24 * 60 + i64::from(user) * 30;
                for step in 0..3u32 {
                    let track = (user + day as u32 * 3 + step) % 12 + 1;
                    events.push(event_of(user, track, start + i64::from(step) * 4));
                }
            }
        }
        events
    }

    fn config(windows: usize) -> WindowConfig {
        WindowConfig {
            windows,
            length: Duration::days(1),
            k: 3,
            ..WindowConfig::default()
        }
    }

    #[test]
    fn windows_are_back_to_back_and_end_with_the_newest_event() {
        let latest = event_of(0, 0, 0).timestamp;
        let bounds = window_bounds(latest, 3, Duration::days(2));

        assert_eq!(bounds.len(), 3);
        assert_eq!(bounds[2].1, latest);
        assert_eq!(bounds[0].0, latest - Duration::days(6));
        for pair in bounds.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
    }

    #[test]
    fn wrong_settings_are_errors() {
        let events = daily_album(2);
        let engine = EngineConfig::lenient();
        let run = |config: WindowConfig| backtest(&events, None, &config, &engine).map(|_| ());

        assert!(matches!(run(config(0)), Err(EvalError::ZeroWindows)));
        let empty = WindowConfig {
            length: Duration::zero(),
            ..config(2)
        };
        assert!(matches!(run(empty), Err(EvalError::EmptyWindow)));
        let zero_k = WindowConfig { k: 0, ..config(2) };
        assert!(matches!(run(zero_k), Err(EvalError::ZeroK)));
        assert!(matches!(
            backtest(&[], None, &config(2), &engine),
            Err(EvalError::NoEvents)
        ));
        let endless = WindowConfig {
            length: Duration::days(100_000_000),
            ..config(2)
        };
        assert!(matches!(run(endless), Err(EvalError::WindowsTooLong)));
    }

    #[test]
    fn every_window_trains_only_on_its_past() {
        let events = daily_album(4);
        let result = backtest(&events, None, &config(3), &EngineConfig::lenient()).unwrap();

        assert_eq!(result.windows.len(), 3);
        let mut seen = 0;
        for window in &result.windows {
            let past = events.iter().filter(|e| e.timestamp <= window.from).count();
            let inside = events
                .iter()
                .filter(|e| e.timestamp > window.from && e.timestamp <= window.to)
                .count();
            assert_eq!(window.train_events, past);
            assert_eq!(window.test_events, inside);
            seen += inside;
        }
        // the windows cover the last three days exactly once
        assert_eq!(seen, 3 * 30);
        assert!(result.windows.iter().all(WindowScores::is_judged));
    }

    #[test]
    fn a_window_before_the_data_has_no_scores() {
        let events = daily_album(2);
        let result = backtest(&events, None, &config(3), &EngineConfig::lenient()).unwrap();

        let oldest = &result.windows[0];
        assert_eq!(oldest.train_events, 0);
        assert!(!oldest.is_judged());
        assert!(oldest.best().is_empty());
        // the summary only counts the windows that were judged
        let hybrid = result
            .stability
            .iter()
            .find(|row| row.model == "hybrid")
            .unwrap();
        assert!(hybrid.windows < 3);
    }

    #[test]
    fn the_summary_has_one_row_per_model_with_sane_numbers() {
        let events = daily_album(5);
        let result = backtest(&events, None, &config(4), &EngineConfig::lenient()).unwrap();

        let names: Vec<&str> = result.stability.iter().map(|row| row.model).collect();
        assert_eq!(names.first(), Some(&"trends"));
        assert_eq!(names.last(), Some(&"hybrid_rules"));
        for row in &result.stability {
            assert!(row.windows <= 4);
            assert!(row.min_ndcg <= row.mean_ndcg + TIE && row.mean_ndcg <= row.max_ndcg + TIE);
            assert!(row.std_ndcg >= 0.0);
            assert!((0.0..=1.0).contains(&row.mean_hit_rate));
            assert!(row.wins <= row.windows);
        }
        // yesterday the neighbours played today's tracks in today's order, so
        // every model finds some, and together they find the most
        let row = |model: &str| {
            result
                .stability
                .iter()
                .find(|row| row.model == model)
                .unwrap()
        };
        assert!(result.stability.iter().all(|row| row.mean_ndcg > 0.0));
        let single = ["trends", "markov", "co_listening", "cf", "mf"];
        assert!(
            single
                .iter()
                .all(|model| row("hybrid").mean_ndcg > row(model).mean_ndcg)
        );
        assert!(row("hybrid").wins > 0);
        let judged = result
            .windows
            .iter()
            .filter(|w| !w.best().is_empty())
            .count();
        let wins: usize = result.stability.iter().map(|row| row.wins).sum();
        assert!(wins >= judged);
    }

    #[test]
    fn statistics_helpers() {
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[1.0, 3.0]), 2.0);
        assert_eq!(std_dev(&[5.0]), 0.0);
        assert!((std_dev(&[1.0, 3.0]) - 1.0).abs() < 1e-12);
        assert!((slope(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)]) - 2.0).abs() < 1e-12);
        assert!((slope(&[(0.0, 0.5), (1.0, 0.3)]) + 0.2).abs() < 1e-12);
        assert_eq!(slope(&[(1.0, 1.0), (1.0, 2.0)]), 0.0);
        assert_eq!(slope(&[(0.0, 1.0)]), 0.0);
    }

    #[test]
    fn ties_give_every_tied_model_a_win() {
        let events = daily_album(4);
        let result = backtest(&events, None, &config(2), &EngineConfig::lenient()).unwrap();
        for window in result.windows.iter().filter(|w| w.is_judged()) {
            let best = window.best();
            let top = window
                .scores
                .iter()
                .map(|s| s.report.ndcg)
                .fold(0.0, f64::max);
            for score in &window.scores {
                assert_eq!(
                    best.contains(&score.model),
                    top > TIE && top - score.report.ndcg <= TIE
                );
            }
        }
    }
}
