//! Commands around the database and measurements: `simulate`, `bench` (R6),
//! `migrate` (R7), `export`, `reset-db` and `etl --append` (E6).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::Duration;

use music_recommendation_system::bench::{SimulationPlan, bench_engine, simulate};
use music_recommendation_system::db::migration;
use music_recommendation_system::db::schema::connect;
use music_recommendation_system::db::sea_repository::SeaOrmRepository;
use music_recommendation_system::db::store::Store;
use music_recommendation_system::engine::EngineConfig;
use music_recommendation_system::etl::append::append;
use music_recommendation_system::evaluation::windows::{self, WindowConfig};
use music_recommendation_system::export::{self, Format};

use crate::cli::{
    BacktestArgs, BenchArgs, EtlArgs, ExportArgs, ExportFormat, ExportKind, MigrateAction,
    MigrateArgs, ResetArgs, SimulateArgs,
};
use crate::commands::{load_csv, open, sqlite_url};

/// Window bounds are printed to the minute.
const MINUTE: &str = "%Y-%m-%d %H:%M";

fn engine_config(lenient: bool) -> EngineConfig {
    if lenient {
        EngineConfig::lenient()
    } else {
        EngineConfig::default()
    }
}

fn existing(db: &str) -> Result<()> {
    if Path::new(db).exists() {
        Ok(())
    } else {
        bail!("database {db} not found, run `etl` first")
    }
}

pub fn simulate_load(args: &SimulateArgs) -> Result<()> {
    existing(&args.db)?;
    println!("writing to {}: run it on a copy of your data", args.db);
    let plan = SimulationPlan {
        users: args.users,
        events: args.events,
        seed: args.seed,
    };
    let report = tokio::runtime::Runtime::new()?.block_on(async {
        let store = Store::open(&sqlite_url(&args.db)).await?;
        simulate(&store, plan).await
    })?;

    println!(
        "{} listeners and {} plays in {:.2}s ({:.0} plays/s)",
        report.users_created,
        report.events_written,
        report.elapsed.as_secs_f64(),
        report.events_written as f64 / report.elapsed.as_secs_f64().max(f64::EPSILON)
    );
    for (name, latencies) in [
        ("create listener", &report.create_user),
        ("store a play", &report.write_event),
        ("read history", &report.read_history),
    ] {
        if let Some(summary) = latencies.summary() {
            println!("{name:<16} {summary}");
        }
    }
    Ok(())
}

pub fn bench(args: &BenchArgs) -> Result<()> {
    let repo = open(&args.data)?;
    let events = repo.all_events()?;
    let config = EngineConfig {
        markov_order: usize::from(args.markov_order),
        ..engine_config(args.lenient)
    };
    let report = bench_engine(&events, &config, args.requests, 1);

    println!("{} plays, {} sessions", report.events, report.sessions);
    for (step, took) in &report.training {
        println!("train {step:<13} {:>9.1} ms", took.as_secs_f64() * 1000.0);
    }
    if let Some(summary) = report.ranking.summary() {
        println!("rank (top 10)       {summary}");
    }
    Ok(())
}

pub fn migrate(args: &MigrateArgs) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let db = connect(&sqlite_url(&args.db))
            .await
            .with_context(|| format!("failed to open {}", args.db))?;
        match args.action {
            MigrateAction::Status => {}
            MigrateAction::Up => migration::up(&db).await?,
            MigrateAction::Down => migration::down(&db, 1).await?,
        }
        for (name, applied) in migration::status(&db).await? {
            println!("{} {name}", if applied { "applied" } else { "pending" });
        }
        Ok(())
    })
}

pub fn export(args: &ExportArgs) -> Result<()> {
    existing(&args.db)?;
    let repo = SeaOrmRepository::open(&sqlite_url(&args.db))
        .with_context(|| format!("failed to open {}", args.db))?;
    let format = match args.format {
        ExportFormat::Csv => Format::Csv,
        ExportFormat::Json => Format::Json,
    };
    let out: Box<dyn Write> = match &args.out {
        Some(path) => Box::new(BufWriter::new(
            File::create(path).with_context(|| format!("cannot create {path}"))?,
        )),
        None => Box::new(std::io::stdout().lock()),
    };

    let rows = match args.what {
        ExportKind::Users => {
            let rows = export::users(&repo)?;
            export::write(&rows, format, out)?;
            rows.len()
        }
        ExportKind::Recommendations => {
            let rows = export::recommendations(&repo, &engine_config(args.lenient), args.top)?;
            export::write(&rows, format, out)?;
            rows.len()
        }
    };
    if let Some(path) = &args.out {
        println!("{rows} rows written to {path}");
    }
    Ok(())
}

pub fn reset(args: &ResetArgs) -> Result<()> {
    existing(&args.db)?;
    if !args.yes {
        bail!(
            "this deletes every listener, track and play in {}; run it again with --yes",
            args.db
        );
    }
    tokio::runtime::Runtime::new()?.block_on(async {
        let db = connect(&sqlite_url(&args.db)).await?;
        // up first, so a database from before migrations is known to the migrator
        migration::up(&db).await?;
        migration::down(&db, u32::MAX).await?;
        migration::up(&db).await?;
        Ok::<_, anyhow::Error>(())
    })?;
    println!("{} is empty, the schema is up to date", args.db);
    Ok(())
}

/// `etl --append`: loads the CSV into memory, then adds what is new.
pub fn etl_append(args: &EtlArgs) -> Result<()> {
    let incoming = load_csv(&args.path)?;
    let mut repo = SeaOrmRepository::open(&sqlite_url(&args.db))
        .with_context(|| format!("failed to open {}", args.db))?;
    let gap = Duration::minutes(i64::try_from(args.gap).unwrap_or(i64::MAX));
    let report = append(&mut repo, &incoming, gap)?;
    println!(
        "added to {}: {} listeners, {} tracks, {} plays ({} plays were there already), {} sessions in all",
        args.db,
        report.new_users,
        report.new_tracks,
        report.new_events,
        report.duplicate_events,
        report.sessions
    );
    Ok(())
}

/// `backtest`: every model on consecutive time windows (W1).
pub fn backtest(args: &BacktestArgs) -> Result<()> {
    let repo = open(&args.data)?;
    let events = repo.all_events()?;
    let tracks = repo.all_tracks()?;
    let users = repo.all_users()?;
    let engine = engine_config(args.lenient);
    let length = i64::try_from(args.days)
        .ok()
        .and_then(Duration::try_days)
        .context("--days is too large")?;
    let config = WindowConfig {
        windows: args.windows,
        length,
        k: args.k,
        session_gap: engine.session_gap,
    };

    let result = windows::backtest(&events, Some((&tracks, &users)), &config, &engine)?;
    for (number, window) in result.windows.iter().enumerate() {
        println!(
            "window {}: {} .. {}, train {} plays, test {} plays, {} users",
            number + 1,
            window.from.format(MINUTE),
            window.to.format(MINUTE),
            window.train_events,
            window.test_events,
            window.users
        );
        if !window.is_judged() {
            println!("  not judged: no plays before it, no plays in it, or nobody with history");
            continue;
        }
        for score in &window.scores {
            println!(
                "  {:<14} ndcg@{k} {:.4}  hit@{k} {:.4}",
                score.model,
                score.report.ndcg,
                score.report.hit_rate,
                k = args.k
            );
        }
        println!("  best: {}", window.best().join(", "));
    }

    if result.stability.is_empty() {
        println!("no window could be judged; try more --days or fewer --windows");
        return Ok(());
    }
    println!();
    println!(
        "{:<14} {:>7} {:>9} {:>7} {:>7} {:>7} {:>8} {:>5}",
        "model", "windows", "mean ndcg", "std", "min", "max", "trend", "wins"
    );
    for row in &result.stability {
        println!(
            "{:<14} {:>7} {:>9.4} {:>7.4} {:>7.4} {:>7.4} {:>+8.4} {:>5}",
            row.model,
            row.windows,
            row.mean_ndcg,
            row.std_ndcg,
            row.min_ndcg,
            row.max_ndcg,
            row.trend,
            row.wins
        );
    }
    println!(
        "trend is the change of NDCG per window: below zero the model does worse on newer plays"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{DEFAULT_DATA, DataArgs};
    use crate::commands::run;
    use music_recommendation_system::db::repository::Repository;

    /// A database loaded from sample.csv, removed when dropped.
    struct TestDb(String);

    impl TestDb {
        fn new(name: &str) -> Self {
            let path = format!("target/{name}-{}.db", std::process::id());
            let _ = std::fs::remove_file(&path);
            run(Some(crate::cli::Command::Etl(EtlArgs {
                path: DEFAULT_DATA.to_string(),
                db: path.clone(),
                gap: 30,
                append: false,
            })))
            .unwrap();
            Self(path)
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn counts(path: &str) -> usize {
        SeaOrmRepository::open(&sqlite_url(path))
            .unwrap()
            .counts()
            .unwrap()
            .events
    }

    #[test]
    fn reset_needs_yes_and_then_empties_the_database() {
        let db = TestDb::new("reset");
        let refused = reset(&ResetArgs {
            db: db.0.clone(),
            yes: false,
        });
        assert!(refused.unwrap_err().to_string().contains("--yes"));
        assert!(counts(&db.0) > 0);

        reset(&ResetArgs {
            db: db.0.clone(),
            yes: true,
        })
        .unwrap();
        assert_eq!(counts(&db.0), 0);
    }

    #[test]
    fn exports_and_migrations_work_on_a_loaded_database() {
        let db = TestDb::new("export");
        let out = format!("{}.csv", db.0);
        export(&ExportArgs {
            what: ExportKind::Recommendations,
            db: db.0.clone(),
            format: ExportFormat::Csv,
            out: Some(out.clone()),
            top: 3,
            lenient: true,
        })
        .unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("user_index,rank,track_index,title,artist,score,reasons"));
        let _ = std::fs::remove_file(&out);

        migrate(&MigrateArgs {
            action: MigrateAction::Status,
            db: db.0.clone(),
        })
        .unwrap();
    }

    #[test]
    fn simulation_and_bench_run_on_sample_data() {
        let db = TestDb::new("simulate");
        let before = counts(&db.0);
        simulate_load(&SimulateArgs {
            db: db.0.clone(),
            users: 2,
            events: 15,
            seed: 1,
        })
        .unwrap();
        assert_eq!(counts(&db.0), before + 15);

        bench(&BenchArgs {
            data: DataArgs {
                path: DEFAULT_DATA.to_string(),
                db: None,
            },
            requests: 5,
            lenient: true,
            markov_order: 2,
        })
        .unwrap();
    }
}
