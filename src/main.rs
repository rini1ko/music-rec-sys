mod db;
mod domain;
mod engine;
mod etl;
mod models;
mod ranker;

use anyhow::{Context, Result};
use chrono::Duration;

use db::memory::MemoryRepository;
use db::repository::Repository;
use engine::{build_ranker, EngineConfig};
use etl::loader::load_from_path;
use models::candidate::RecommendationContext;
use models::co_listening::CoListeningConfig;

const DATA_PATH: &str = "data/sample.csv";

fn main() -> Result<()> {
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