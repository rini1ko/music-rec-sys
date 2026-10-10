use std::env;

use anyhow::{Context, Result, bail};
use chrono::Duration;

use music_recommendation_system::db::memory::MemoryRepository;
use music_recommendation_system::db::repository::Repository;
use music_recommendation_system::engine::{EngineConfig, Weights, build_ranker};
use music_recommendation_system::etl::loader::load_from_path;
use music_recommendation_system::evaluation::split::SplitMode;
use music_recommendation_system::evaluation::tuning::tune;
use music_recommendation_system::evaluation::{EvalConfig, Evaluator};
use music_recommendation_system::models::candidate::RecommendationContext;
use music_recommendation_system::models::cf::CfConfig;
use music_recommendation_system::models::co_listening::CoListeningConfig;

const DATA_PATH: &str = "data/sample.csv";
const USAGE: &str = "commands: eval [path] [--global], tune [path] [--global]";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = Options::parse(&args[args.len().min(1)..]);

    match args.first().map(String::as_str) {
        None => run_demo(),
        Some("eval") => run_eval(&options),
        Some("tune") => run_tune(&options),
        Some(other) => bail!("unknown command `{other}`, {USAGE}"),
    }
}

struct Options {
    path: String,
    split: SplitMode,
}

impl Options {
    /// `path` is the first argument that is not a flag.
    fn parse(args: &[String]) -> Self {
        Options {
            path: args
                .iter()
                .find(|a| !a.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| DATA_PATH.to_string()),
            split: if args.iter().any(|a| a == "--global") {
                SplitMode::Global
            } else {
                SplitMode::PerUser
            },
        }
    }
}

/// Loads the CSV and prepares an evaluator: models are trained once here.
fn prepare(options: &Options) -> Result<Evaluator> {
    let mut repo = MemoryRepository::default();
    let report = load_from_path(&options.path, &mut repo, Duration::minutes(30))
        .with_context(|| format!("failed to load {}", options.path))?;
    println!(
        "rows loaded: {}, skipped: {}",
        report.rows_loaded, report.rows_skipped
    );

    let events = repo.all_events()?;
    let eval = EvalConfig {
        split: options.split,
        ..EvalConfig::default()
    };
    let evaluator = Evaluator::new(&events, &eval, &EngineConfig::default())?;

    let summary = evaluator.run();
    println!(
        "split: {:?}, train events: {}, test events: {}",
        options.split, summary.train_events, summary.test_events
    );
    println!(
        "users evaluated: {} (skipped: {} without history, {} with nothing new)",
        evaluator.case_count(),
        summary.skipped_cold_start,
        summary.skipped_nothing_new
    );
    Ok(evaluator)
}

/// Compares every model alone and the hybrid.
fn run_eval(options: &Options) -> Result<()> {
    let mut evaluator = prepare(options)?;

    let presets = [
        (
            "trends only",
            Weights {
                trends: 1.0,
                ..Weights::ZERO
            },
        ),
        (
            "markov only",
            Weights {
                markov: 1.0,
                ..Weights::ZERO
            },
        ),
        (
            "co-listening only",
            Weights {
                co_listening: 1.0,
                ..Weights::ZERO
            },
        ),
        (
            "cf only",
            Weights {
                cf: 1.0,
                ..Weights::ZERO
            },
        ),
        ("hybrid (default)", EngineConfig::default().weights),
    ];

    for (name, weights) in presets {
        evaluator.set_weights(weights);
        println!("{name:<18} {}", evaluator.run());
    }
    Ok(())
}

/// Tries many weight combinations: picks on half of the users, checks on the other half.
fn run_tune(options: &Options) -> Result<()> {
    let mut evaluator = prepare(options)?;
    let default = EngineConfig::default().weights;
    let tuned = tune(&mut evaluator, 10, default, 5);

    println!("\ntop 5 on the tuning half:");
    for (weights, report) in &tuned.top {
        println!("  {weights}   ndcg {:.4}  hit {:.4}", report.ndcg, report.hit_rate);
    }

    println!("\n{:<26} {:>12} {:>12}", "", "tuning half", "other half");
    println!(
        "{:<26} {:>12.4} {:>12.4}",
        "default weights (ndcg)", tuned.default_tuning.ndcg, tuned.default_holdout.ndcg
    );
    println!(
        "{:<26} {:>12.4} {:>12.4}",
        "best weights (ndcg)", tuned.best_tuning.ndcg, tuned.best_holdout.ndcg
    );
    println!("\nbest: {}", tuned.best);
    println!("the number in the last column is the honest one");
    Ok(())
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
        cf: CfConfig {
            min_overlap: 1,
            ..CfConfig::default()
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