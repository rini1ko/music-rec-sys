use std::env;

use anyhow::{Context, Result, bail};
use chrono::Duration;

use music_recommendation_system::db::memory::MemoryRepository;
use music_recommendation_system::db::repository::Repository;
use music_recommendation_system::engine::{EngineConfig, Weights, build_ranker};
use music_recommendation_system::etl::loader::load_from_path;
use music_recommendation_system::evaluation::{EvalConfig, evaluate};
use music_recommendation_system::models::candidate::RecommendationContext;
use music_recommendation_system::models::co_listening::CoListeningConfig;

const DATA_PATH: &str = "data/sample.csv";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => run_demo(),
        Some("eval") => run_eval(args.get(1).map_or(DATA_PATH, String::as_str)),
        Some(other) => bail!("unknown command `{other}`, available: eval [path]"),
    }
}

/// Compares every model alone and the hybrid on a time-based split.
fn run_eval(path: &str) -> Result<()> {
    let mut repo = MemoryRepository::default();
    let report = load_from_path(path, &mut repo, Duration::minutes(30))
        .with_context(|| format!("failed to load {path}"))?;
    println!(
        "rows loaded: {}, skipped: {}",
        report.rows_loaded, report.rows_skipped
    );

    let events = repo.all_events()?;
    let eval = EvalConfig::default();

    let presets = [
        ("trends only", only(1.0, 0.0, 0.0)),
        ("markov only", only(0.0, 1.0, 0.0)),
        ("co-listening only", only(0.0, 0.0, 1.0)),
        ("hybrid (default)", EngineConfig::default().weights),
    ];

    for (name, weights) in presets {
        let engine = EngineConfig::default().with_weights(weights);
        let result = evaluate(&events, &eval, &engine)?;
        println!("{name:<18} {result}");
    }
    Ok(())
}

fn only(trends: f64, markov: f64, co_listening: f64) -> Weights {
    Weights {
        trends,
        markov,
        co_listening,
    }
}

fn run_demo() -> Result<()> {
    let mut repo = MemoryRepository::default();

    let report = load_from_path(DATA_PATH, &mut repo, Duration::minutes(30))
        .with_context(|| format!("failed to load {DATA_PATH}"))?;

    println!(
        "rows loaded: {}, skipped: {}",
        report.rows_loaded, report.rows_skipped
    );
    for message in &report.first_errors {
        println!("  skipped row: {message}");
    }

    let counts = repo.counts()?;
    println!(
        "users: {}, tracks: {}, events: {}, sessions: {}",
        counts.users, counts.tracks, counts.events, counts.sessions
    );

    // lenient thresholds, because the sample file is tiny
    let config = EngineConfig {
        co_listening: CoListeningConfig {
            min_cooccurrence: 1,
            min_jaccard: 0.0,
            ..CoListeningConfig::default()
        },
        ..EngineConfig::default()
    };
    let ranker = build_ranker(&repo, &config)?;

    let user_index = 2;
    let country = repo
        .get_user(user_index)?
        .and_then(|user| user.country)
        .unwrap_or_else(|| "unknown".to_string());
    let context = RecommendationContext {
        history: repo.user_history(user_index)?,
    };

    println!(
        "recommendations for user {user_index} ({country}), history: {:?}",
        context.history
    );
    for rec in ranker.recommend(&context, 3) {
        let title = repo
            .get_track(rec.track_index)?
            .and_then(|track| track.track_name)
            .unwrap_or_else(|| "unknown track".to_string());
        println!(
            "  {title} (track {})  score {:.3}  ({})",
            rec.track_index,
            rec.score,
            rec.explain()
        );
    }

    Ok(())
}