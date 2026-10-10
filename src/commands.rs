use anyhow::{Context, Result, bail};
use chrono::Duration;
use std::path::Path;
use std::fs::File;
use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

use music_recommendation_system::db::memory::MemoryRepository;
use music_recommendation_system::db::repository::Repository;
use music_recommendation_system::engine::{EngineConfig, Weights, build_ranker};
use music_recommendation_system::evaluation::split::SplitMode;
use music_recommendation_system::evaluation::tuning::tune;
use music_recommendation_system::evaluation::{EvalConfig, Evaluator};
use music_recommendation_system::models::candidate::RecommendationContext;
use music_recommendation_system::models::cf::CfConfig;
use music_recommendation_system::models::co_listening::CoListeningConfig;
use music_recommendation_system::db::sea_repository::SeaOrmRepository;
use music_recommendation_system::etl::loader::{LoadReport, load_from_path, load_from_reader};

use crate::cli::{Command, DEFAULT_DATA, DataArgs, EtlArgs, EvalArgs, RecommendArgs, TuneArgs};

pub fn run(command: Option<Command>) -> Result<()> {
    match command {
        None => demo(),
        Some(Command::Stats(args)) => stats(&args),
        Some(Command::Recommend(args)) => recommend(&args),
        Some(Command::Eval(args)) => eval(&args),
        Some(Command::Tune(args)) => tune_weights(&args),
        Some(Command::Etl(args)) => etl(&args),
    }
}

fn print_report(report: &LoadReport) {
    println!(
        "rows loaded: {}, skipped: {}",
        report.rows_loaded, report.rows_skipped
    );
    for message in &report.first_errors {
        println!("  skipped row: {message}");
    }
}

fn load_csv(path: &str) -> Result<MemoryRepository> {
    let mut repo = MemoryRepository::default();
    let report = load_from_path(path, &mut repo, Duration::minutes(30))
        .with_context(|| format!("failed to load {path}"))?;
    print_report(&report);
    Ok(repo)
}

/// The database if `--db` is given, otherwise the CSV loaded into memory.
fn open(args: &DataArgs) -> Result<Box<dyn Repository>> {
    let Some(db) = &args.db else {
        return Ok(Box::new(load_csv(&args.path)?));
    };
    if !Path::new(db).exists() {
        bail!("database {db} not found, run `etl` first");
    }
    let repo = SeaOrmRepository::open(&sqlite_url(db))
        .with_context(|| format!("failed to open {db}"))?;
    println!("reading from {db}");
    Ok(Box::new(repo))
}

/// Relative paths only: a Windows `C:\...` path does not fit a sqlite URL.
fn sqlite_url(path: &str) -> String {
    format!("sqlite://{path}?mode=rwc")
}

fn progress_bar(total_bytes: u64) -> ProgressBar {
    let bar = ProgressBar::new(total_bytes);
    bar.set_style(
        ProgressStyle::with_template("reading [{bar:40}] {bytes}/{total_bytes} eta {eta}")
            .expect("the template is valid")
            .progress_chars("=> "),
    );
    bar
}

fn etl(args: &EtlArgs) -> Result<()> {
    let mut repo = SeaOrmRepository::open(&sqlite_url(&args.db))
        .with_context(|| format!("failed to open {}", args.db))?;

    // loading twice would duplicate every event and session
    if repo.counts()?.events > 0 {
        bail!(
            "{} already contains data, delete it or pick another --db",
            args.db
        );
    }

    let gap = Duration::minutes(args.gap as i64);
    let file = File::open(&args.path).with_context(|| format!("failed to open {}", args.path))?;
    let bar = progress_bar(file.metadata()?.len());
    let started = Instant::now();

    let report = load_from_reader(bar.wrap_read(file), &mut repo, gap)
        .with_context(|| format!("failed to load {}", args.path))?;
    bar.finish_and_clear();

    print_report(&report);
    println!("done in {:.1}s", started.elapsed().as_secs_f32());
    
    let counts = repo.counts()?;
    println!(
        "stored in {}: users {}, tracks {}, events {}, sessions {}",
        args.db, counts.users, counts.tracks, counts.events, counts.sessions
    );
    Ok(())
}

fn demo() -> Result<()> {
    println!("demo on {DEFAULT_DATA} (run with --help to see all commands)\n");
    recommend(&RecommendArgs {
        data: DataArgs {
            path: DEFAULT_DATA.to_string(),
            db: None,
        },
        user: 2,
        top: 3,
        lenient: true,
    })
}

fn stats(args: &DataArgs) -> Result<()> {
    let repo = open(args)?;
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
    let repo = open(&args.data)?;

    let Some(user) = repo.get_user(args.user)? else {
        bail!("user {} is not in {}", args.user, args.data.path);
    };

    let config = if args.lenient {
        lenient_config()
    } else {
        EngineConfig::default()
    };
    let ranker = build_ranker(repo.as_ref(), &config)?;

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
    let repo = open(&args.data)?;
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
            db: None,
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
            db: None,
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
    #[test]
    fn etl_fills_a_database_only_once() {
        let path = format!("target/etl-test-{}.db", std::process::id());
        let _ = std::fs::remove_file(&path);
        let args = EtlArgs {
            path: DEFAULT_DATA.to_string(),
            db: path.clone(),
            gap: 30,
        };

        etl(&args).unwrap();
        let error = etl(&args).unwrap_err();
        assert!(error.to_string().contains("already contains data"));

        let _ = std::fs::remove_file(&path);
    }
        #[test]
    fn commands_can_read_from_the_database() {
        let path = format!("target/db-read-test-{}.db", std::process::id());
        let _ = std::fs::remove_file(&path);
        etl(&EtlArgs {
            path: DEFAULT_DATA.to_string(),
            db: path.clone(),
            gap: 30,
        })
        .unwrap();

        let from_db = DataArgs {
            path: DEFAULT_DATA.to_string(),
            db: Some(path.clone()),
        };
        stats(&from_db).unwrap();
        recommend(&RecommendArgs {
            data: from_db,
            user: 2,
            top: 3,
            lenient: true,
        })
        .unwrap();

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_database_is_reported() {
        let args = DataArgs {
            path: DEFAULT_DATA.to_string(),
            db: Some("target/no-such.db".to_string()),
        };
        let error = stats(&args).unwrap_err();
        assert!(error.to_string().contains("run `etl` first"));
    }
}