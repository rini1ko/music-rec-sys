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

use crate::cli::{Command, DEFAULT_DATA, DataArgs, EvalArgs, RecommendArgs, TuneArgs};

pub fn run(command: Option<Command>) -> Result<()> {
    match command {
        None => demo(),
        Some(Command::Stats(args)) => stats(&args),
        Some(Command::Recommend(args)) => recommend(&args),
        Some(Command::Eval(args)) => eval(&args),
        Some(Command::Tune(args)) => tune_weights(&args),
    }
}

/// Reads the CSV into memory and reports what happened to its rows.
fn load(path: &str) -> Result<MemoryRepository> {
    let mut repo = MemoryRepository::default();
    let report = load_from_path(path, &mut repo, Duration::minutes(30))
        .with_context(|| format!("failed to load {path}"))?;

    println!(
        "rows loaded: {}, skipped: {}",
        report.rows_loaded, report.rows_skipped
    );
    for message in &report.first_errors {
        println!("  skipped row: {message}");
    }
    Ok(repo)
}

fn demo() -> Result<()> {
    println!("demo on {DEFAULT_DATA} (run with --help to see all commands)\n");
    recommend(&RecommendArgs {
        data: DataArgs {
            path: DEFAULT_DATA.to_string(),
        },
        user: 2,
        top: 3,
        lenient: true,
    })
}

fn stats(args: &DataArgs) -> Result<()> {
    let repo = load(&args.path)?;
    let counts = repo.counts()?;
    println!(
        "users: {}, tracks: {}, events: {}, sessions: {}",
        counts.users, counts.tracks, counts.events, counts.sessions
    );
    Ok(())
}

/// Small files do not have enough overlap for the default thresholds.
fn lenient_config() -> EngineConfig {
    EngineConfig {
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
    }
}

fn recommend(args: &RecommendArgs) -> Result<()> {
    let repo = load(&args.data.path)?;

    let Some(user) = repo.get_user(args.user)? else {
        bail!("user {} is not in {}", args.user, args.data.path);
    };

    let config = if args.lenient {
        lenient_config()
    } else {
        EngineConfig::default()
    };
    let ranker = build_ranker(&repo, &config)?;

    let history = repo.user_history(args.user)?;
    let country = user.country.as_deref().unwrap_or("unknown country");
    println!(
        "\nrecommendations for user {} ({country}), {} tracks in history",
        args.user,
        history.len()
    );

    let recommendations = ranker.recommend(&RecommendationContext { history }, args.top);
    if recommendations.is_empty() {
        println!("  nothing to recommend, try --lenient for a small file");
        return Ok(());
    }

    for (position, rec) in recommendations.iter().enumerate() {
        let track = repo.get_track(rec.track_index)?;
        let title = track
            .as_ref()
            .and_then(|t| t.track_name.as_deref())
            .unwrap_or("unknown track");
        let artist = track
            .as_ref()
            .and_then(|t| t.artist_name.as_deref())
            .unwrap_or("unknown artist");

        println!("{:>3}. {title} - {artist}  (score {:.3})", position + 1, rec.score);
        println!("     {}", rec.explain());
    }
    Ok(())
}

/// Loads the CSV and prepares an evaluator: the models are trained once here.
fn prepare(args: &EvalArgs) -> Result<Evaluator> {
    let repo = load(&args.data.path)?;
    let events = repo.all_events()?;

    let split = if args.global {
        SplitMode::Global
    } else {
        SplitMode::PerUser
    };
    let config = EvalConfig {
        k: args.k,
        split,
        ..EvalConfig::default()
    };
    let evaluator = Evaluator::new(&events, &config, &EngineConfig::default())?;

    let summary = evaluator.run();
    println!(
        "split: {split:?}, train events: {}, test events: {}",
        summary.train_events, summary.test_events
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
fn eval(args: &EvalArgs) -> Result<()> {
    let mut evaluator = prepare(args)?;

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
fn tune_weights(args: &TuneArgs) -> Result<()> {
    let mut evaluator = prepare(&args.eval)?;
    let default = EngineConfig::default().weights;
    let tuned = tune(&mut evaluator, args.steps, default, args.show);

    println!("\ntop {} on the tuning half:", tuned.top.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DataArgs {
        DataArgs {
            path: DEFAULT_DATA.to_string(),
        }
    }

    fn recommend_args(user: u32) -> RecommendArgs {
        RecommendArgs {
            data: sample(),
            user,
            top: 3,
            lenient: true,
        }
    }

    #[test]
    fn demo_runs_on_the_sample_file() {
        demo().unwrap();
    }

    #[test]
    fn recommend_works_for_a_known_user() {
        recommend(&recommend_args(2)).unwrap();
    }

    #[test]
    fn recommend_explains_an_unknown_user() {
        let error = recommend(&recommend_args(999_999)).unwrap_err();
        assert!(error.to_string().contains("999999"));
    }

    #[test]
    fn missing_file_error_names_the_file() {
        let error = stats(&DataArgs {
            path: "data/no-such-file.csv".to_string(),
        })
        .unwrap_err();
        assert!(error.to_string().contains("no-such-file.csv"));
    }

    #[test]
    fn stats_works_on_the_sample_file() {
        stats(&sample()).unwrap();
    }

    #[test]
    fn eval_and_tune_run_on_the_sample_file() {
        let eval_args = || EvalArgs {
            data: sample(),
            global: false,
            k: 5,
        };
        eval(&eval_args()).unwrap();
        tune_weights(&TuneArgs {
            eval: eval_args(),
            steps: 2,
            show: 3,
        })
        .unwrap();
    }
}