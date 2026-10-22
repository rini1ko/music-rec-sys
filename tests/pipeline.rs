//! The library without HTTP: a CSV file goes through the loader, the models
//! train on it, and the evaluation and the weight search judge them.

mod common;

use chrono::Duration;

use music_recommendation_system::db::memory::MemoryRepository;
use music_recommendation_system::db::repository::Repository;
use music_recommendation_system::engine::{EngineConfig, build_ranker};
use music_recommendation_system::etl::loader::load_from_reader;
use music_recommendation_system::evaluation::compare::compare_models;
use music_recommendation_system::evaluation::tuning::tune;
use music_recommendation_system::evaluation::{EvalConfig, Evaluator};
use music_recommendation_system::models::candidate::RecommendationContext;

use common::{LISTENERS, PLAYS, events, tracks, users};

const HEADER: &str =
    "user_index,artist_index,track_index,timestamp,artist_name,track_name,gender,age,country";

/// The shared fixture written out in the Last.fm CSV layout.
fn fixture_csv() -> String {
    let tracks = tracks();
    let users = users();
    let mut lines = vec![HEADER.to_string()];
    for event in events() {
        let track = tracks
            .iter()
            .find(|t| t.track_index == event.track_index)
            .unwrap();
        let user = users
            .iter()
            .find(|u| u.user_index == event.user_index)
            .unwrap();
        lines.push(format!(
            "{},{},{},{},{},{},,,{}",
            event.user_index,
            track.artist_index,
            track.track_index,
            event.timestamp.format("%Y-%m-%dT%H:%M:%S"),
            track.artist_name.as_deref().unwrap_or_default(),
            track.track_name.as_deref().unwrap_or_default(),
            user.country.as_deref().unwrap_or_default(),
        ));
    }
    lines.join("\n")
}

fn loaded() -> MemoryRepository {
    let mut repo = MemoryRepository::default();
    load_from_reader(fixture_csv().as_bytes(), &mut repo, Duration::minutes(30)).unwrap();
    repo
}

#[test]
fn the_loader_keeps_good_rows_and_counts_bad_ones() {
    let broken = format!(
        "{}\nnot-a-user,1,10,2009-05-04T12:00:00,A,B,,,",
        fixture_csv()
    );
    let late = "1,1,10,yesterday,Okean Elzy,Song 10,,,Ukraine";
    let text = format!("{broken}\n{late}");

    let mut repo = MemoryRepository::default();
    let report = load_from_reader(text.as_bytes(), &mut repo, Duration::minutes(30)).unwrap();

    assert_eq!(report.rows_loaded, PLAYS);
    assert_eq!(report.rows_skipped, 2);
    assert_eq!(report.first_errors.len(), 2);
    assert_eq!(report.users, LISTENERS as usize);
    assert_eq!(report.tracks, tracks().len());
    assert_eq!(report.sessions, LISTENERS as usize);

    let counts = repo.counts().unwrap();
    assert_eq!(counts.events, PLAYS);
    let user = repo.get_user(2).unwrap().unwrap();
    assert_eq!(user.country.as_deref(), Some("Germany"));
}

#[test]
fn a_ranker_trained_on_the_file_continues_the_album() {
    let repo = loaded();
    let ranker = build_ranker(&repo, &EngineConfig::lenient()).unwrap();
    let context = RecommendationContext { history: vec![10] };

    let first = ranker.recommend(&context, 3);
    let order: Vec<u32> = first.iter().map(|item| item.track_index).collect();
    assert_eq!(order.first(), Some(&11), "{order:?}");
    assert!(!order.contains(&10));
    assert!(!first[0].explain().is_empty());

    // training is deterministic: the same data gives the same list
    let again = build_ranker(&repo, &EngineConfig::lenient()).unwrap();
    let repeated: Vec<u32> = again
        .recommend(&context, 3)
        .iter()
        .map(|item| item.track_index)
        .collect();
    assert_eq!(order, repeated);
}

#[test]
fn every_model_gets_scores_between_zero_and_one() {
    let repo = loaded();
    let config = EvalConfig {
        k: 3,
        ..EvalConfig::default()
    };
    let engine = EngineConfig::lenient();
    let mut evaluator = Evaluator::new(&repo.all_events().unwrap(), &config, &engine)
        .unwrap()
        .with_catalog(&repo.all_tracks().unwrap(), &repo.all_users().unwrap());

    let scores = compare_models(&mut evaluator, engine.weights);
    let names: Vec<&str> = scores.iter().map(|score| score.model).collect();
    assert_eq!(
        names,
        [
            "trends",
            "markov",
            "markov_2",
            "co_listening",
            "cf",
            "mf",
            "hybrid",
            "hybrid_rules"
        ]
    );
    for score in &scores {
        let report = &score.report;
        assert!(report.users_evaluated > 0, "{}", score.model);
        for value in [
            report.hit_rate,
            report.ndcg,
            report.precision,
            report.recall,
            report.coverage,
        ] {
            assert!((0.0..=1.0).contains(&value), "{}: {report}", score.model);
        }
        assert!(report.novelty >= 0.0);
    }

    // listeners 1-5 hold out track 13, so no model saw it and none can hit it;
    // the hybrid still shows tracks, while the Markov chain has nothing after 12
    let report = |model: &str| {
        &scores
            .iter()
            .find(|score| score.model == model)
            .unwrap()
            .report
    };
    assert!(report("hybrid").coverage > 0.0, "{}", report("hybrid"));
    assert_eq!(report("hybrid").hit_rate, 0.0);
    assert_eq!(report("markov").coverage, 0.0, "{}", report("markov"));
}

#[test]
fn the_weight_search_returns_weights_that_add_up_to_one() {
    let repo = loaded();
    let engine = EngineConfig::lenient();
    let mut evaluator =
        Evaluator::new(&repo.all_events().unwrap(), &EvalConfig::default(), &engine).unwrap();

    let tuned = tune(&mut evaluator, 2, engine.weights, 3);
    let best = tuned.best;
    let total = best.trends + best.markov + best.co_listening + best.cf + best.mf;
    assert!((total - 1.0).abs() < 1e-9, "{best:?}");
    assert_eq!(tuned.top.len(), 3);
    assert_eq!(tuned.top[0].0, best);
    assert!(tuned.best_tuning.ndcg >= tuned.default_tuning.ndcg);
}

#[test]
fn an_empty_file_is_an_error_for_the_evaluation() {
    let mut repo = MemoryRepository::default();
    let report = load_from_reader(HEADER.as_bytes(), &mut repo, Duration::minutes(30)).unwrap();
    assert_eq!(report.rows_loaded, 0);
    let events = repo.all_events().unwrap();
    assert!(Evaluator::new(&events, &EvalConfig::default(), &EngineConfig::lenient()).is_err());
}
