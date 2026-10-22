//! The whole recommender, rules included (E5). The plain run judges the
//! weighted ranking only; this one adds what users of the API get on top:
//! the cold start for listeners without training plays, the bonus for their
//! favourite artist and the cap of tracks per artist.

use crate::models::catalog::Catalog;
use crate::ranker::{Ranker, RankerConfig};
use crate::recommender::{Recommender, WeightOverrides};

use super::{EvalReport, Evaluator};

impl Evaluator {
    /// Judges the recommender with its rules on every user, also the ones
    /// [`Evaluator::run`] has to skip because they had no training plays.
    /// Without [`Evaluator::with_catalog`] the rules know no artists or
    /// countries and the cold start falls back to tracks popular overall.
    pub fn run_rules(&mut self) -> EvalReport {
        // the recommender owns its ranker, so the ranker is lent for this run
        let ranker = std::mem::replace(&mut self.ranker, Ranker::new(RankerConfig::default()));
        let catalog = self.catalog.take().unwrap_or_else(|| {
            Catalog::build(&[], &[], &self.train_data, self.rules.cold_start_pool)
        });
        let recommender = Recommender::new(ranker, catalog, self.rules.clone());

        let cases = self.cases.iter().chain(&self.cold_cases);
        let report = self.judge(cases, 0, |case| {
            let country = self.countries.get(&case.user).map(String::as_str);
            recommender
                .recommend(
                    case.history.clone(),
                    country,
                    self.k,
                    WeightOverrides::default(),
                )
                .into_iter()
                .map(|item| item.track_index)
                .collect()
        });

        let (ranker, catalog) = recommender.into_parts();
        self.ranker = ranker;
        self.catalog = Some(catalog);
        report
    }

    /// Users the rules can judge although the plain ranking skips them.
    pub fn cold_case_count(&self) -> usize {
        self.cold_cases.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::User;
    use crate::domain::test_utils::{event_of, named_track, user_from};
    use crate::engine::EngineConfig;
    use crate::evaluation::split::SplitMode;
    use crate::evaluation::{EvalConfig, Evaluator};

    /// Four listeners from the UK play 1 -> 2 -> 3. User 9 (UK) has no plays
    /// before the cutoff and plays 3 after it: only the cold start can find it.
    fn events() -> Vec<crate::domain::ListenEvent> {
        let mut events = Vec::new();
        for user in 0..4 {
            let base = i64::from(user) * 100;
            events.push(event_of(user, 1, base));
            events.push(event_of(user, 2, base + 1));
            events.push(event_of(user, 3, base + 2));
            events.push(event_of(user, 3, base + 3));
        }
        events.push(event_of(9, 3, 1000));
        events
    }

    fn config() -> EvalConfig {
        EvalConfig {
            k: 3,
            test_fraction: 1.0 / 17.0,
            split: SplitMode::Global,
            ..EvalConfig::default()
        }
    }

    #[test]
    fn the_rules_also_serve_listeners_without_history() {
        let users: Vec<User> = (0..4)
            .chain([9])
            .map(|index| user_from(index, "UK"))
            .collect();
        let tracks: Vec<_> = (1..=3)
            .map(|index| named_track(index, index, "t", "a"))
            .collect();
        let mut evaluator = Evaluator::new(&events(), &config(), &EngineConfig::lenient())
            .unwrap()
            .with_catalog(&tracks, &users);

        let plain = evaluator.run();
        assert_eq!(plain.users_evaluated, 0);
        assert_eq!(plain.skipped_cold_start, 1);
        assert_eq!(evaluator.cold_case_count(), 1);

        let rules = evaluator.run_rules();
        assert_eq!(rules.users_evaluated, 1);
        assert_eq!(rules.skipped_cold_start, 0);
        // 3 is the most played track in the UK, so the cold start hits it first
        assert!((rules.hit_rate - 1.0).abs() < 1e-9);
        assert!((rules.ndcg - 1.0).abs() < 1e-9);

        // the ranker came back: the plain run still works and gives the same
        assert_eq!(evaluator.run(), plain);
    }
}
