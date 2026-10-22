use clap::{Args, Parser, Subcommand};
use music_recommendation_system::config::Secret;

pub const DEFAULT_DATA: &str = "data/sample.csv";
pub const DEFAULT_DB: &str = "data/music.db";
/// Hybrid music recommender trained on Last.fm listening history.
///
/// Run without a command to see a small demo on data/sample.csv.
#[derive(Debug, Parser)]
#[command(name = "music-recommendation-system", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Load a CSV file and print statistics about it
    Stats(DataArgs),
    /// Recommend tracks for one user
    Recommend(RecommendArgs),
    /// Compare every model on a train/test split
    Eval(EvalArgs),
    /// Search for the best model weights
    Tune(TuneArgs),
    /// Judge every model on consecutive time windows, each trained on the plays before it
    Backtest(BacktestArgs),
    /// Load a CSV file into the SQLite database
    Etl(EtlArgs),
    /// Start the HTTP API
    Serve(ServeArgs),
    /// Create listeners and plays through the database layer and time every call
    Simulate(SimulateArgs),
    /// Time the training of every model and the ranking
    Bench(BenchArgs),
    /// Show or apply the schema migrations
    Migrate(MigrateArgs),
    /// Write listeners or recommendations as CSV or JSON
    Export(ExportArgs),
    /// Delete every row of a database and create the schema again
    ResetDb(ResetArgs),
}

#[derive(Debug, Args)]
pub struct DataArgs {
    /// CSV file with listening events
    #[arg(default_value = DEFAULT_DATA)]
    pub path: String,

    /// Read from this SQLite database (made by `etl`) instead of the CSV
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Debug, Args)]
pub struct EtlArgs {
    /// CSV file with listening events
    #[arg(default_value = DEFAULT_DATA)]
    pub path: String,

    /// SQLite database file, created if missing
    #[arg(long, default_value = DEFAULT_DB)]
    pub db: String,

    /// Minutes of silence that start a new session
    #[arg(long, default_value_t = 30, value_parser = at_least_one)]
    pub gap: usize,

    /// Add to a database that has data: skip known plays, cut the sessions again
    #[arg(long)]
    pub append: bool,
}

#[derive(Debug, Args)]
pub struct RecommendArgs {
    #[command(flatten)]
    pub data: DataArgs,

    /// Index of the user to recommend for (the user_index column)
    #[arg(short, long)]
    pub user: u32,

    /// How many tracks to recommend
    #[arg(short, long, default_value_t = 10, value_parser = at_least_one)]
    pub top: usize,

    /// Low similarity thresholds, for tiny files such as the sample
    #[arg(long)]
    pub lenient: bool,
}

#[derive(Debug, Args)]
pub struct EvalArgs {
    #[command(flatten)]
    pub data: DataArgs,

    /// One global time cutoff instead of holding out each user's newest events
    #[arg(long)]
    pub global: bool,

    /// Length of the recommendation list that is judged
    #[arg(short = 'k', long = "k", default_value_t = 10, value_parser = at_least_one)]
    pub k: usize,
}

#[derive(Debug, Args)]
pub struct TuneArgs {
    #[command(flatten)]
    pub eval: EvalArgs,

    /// Weights are tried in steps of 1/STEPS (10 means 1001 combinations of the five models)
    #[arg(long, default_value_t = 10, value_parser = steps_in_range)]
    pub steps: usize,

    /// How many of the best combinations to print
    #[arg(long, default_value_t = 5, value_parser = at_least_one)]
    pub show: usize,
}

#[derive(Debug, Args)]
pub struct BacktestArgs {
    #[command(flatten)]
    pub data: DataArgs,

    /// How many windows; the newest one ends with the newest play
    #[arg(long, default_value_t = 4, value_parser = at_least_one)]
    pub windows: usize,

    /// Length of one window in days
    #[arg(long, default_value_t = 7, value_parser = at_least_one)]
    pub days: usize,

    /// Length of the recommendation list that is judged
    #[arg(short = 'k', long = "k", default_value_t = 10, value_parser = at_least_one)]
    pub k: usize,

    /// Low similarity thresholds, for tiny files such as the sample
    #[arg(long)]
    pub lenient: bool,
}

#[derive(Debug, Default, Args)]
/// Every value can also be set in `config.toml` or in a `MUSIC_*` variable.
/// Priority: command line > environment > file > default.
pub struct ServeArgs {
    /// Settings file [default: config.toml, if it exists]
    #[arg(long, env = "MUSIC_CONFIG")]
    pub config: Option<String>,

    /// SQLite database made by `etl` [default: data/music.db]
    #[arg(long, env = "MUSIC_DB")]
    pub db: Option<String>,

    /// Address to listen on [default: 127.0.0.1:3000]
    #[arg(long, env = "MUSIC_ADDR")]
    pub addr: Option<String>,

    /// Directory for the JSON log files [default: logs]
    #[arg(long, env = "MUSIC_LOG_DIR")]
    pub log_dir: Option<String>,

    /// Relaxed thresholds for tiny datasets such as sample.csv
    #[arg(long, env = "MUSIC_LENIENT", num_args = 0..=1, default_missing_value = "true")]
    pub lenient: Option<bool>,

    /// Retrain the models every N minutes, 0 means only on request [default: 0]
    #[arg(long, env = "MUSIC_RETRAIN_MINUTES")]
    pub retrain_minutes: Option<u64>,

    /// Browser origins allowed to call the API (repeat the flag, or separate with commas)
    #[arg(
        long = "cors-origin",
        env = "MUSIC_CORS_ORIGINS",
        value_delimiter = ','
    )]
    pub cors_origins: Vec<String>,

    /// Key required in x-api-key for writes and /model/* actions [default: none, all open]
    #[arg(long, env = "MUSIC_API_KEY", hide_env_values = true)]
    pub api_key: Option<Secret>,

    /// Average requests per second per client, 0 turns the limit off [default: 20]
    #[arg(long, env = "MUSIC_RATE_PER_SECOND")]
    pub rate_per_second: Option<f64>,

    /// Requests one client may send at once [default: 40]
    #[arg(long, env = "MUSIC_RATE_BURST")]
    pub rate_burst: Option<u32>,

    /// Seconds a request may take before it fails with 503 [default: 30]
    #[arg(long, env = "MUSIC_REQUEST_TIMEOUT_SECS")]
    pub request_timeout_secs: Option<u64>,

    /// Largest request body in bytes [default: 65536]
    #[arg(long, env = "MUSIC_BODY_LIMIT_BYTES")]
    pub body_limit_bytes: Option<usize>,

    /// Recommendation lists kept ready, 0 turns the cache off [default: 1024]
    #[arg(long, env = "MUSIC_CACHE_CAPACITY")]
    pub cache_capacity: Option<usize>,
}

#[derive(Debug, Args)]
pub struct SimulateArgs {
    /// Database to write to; use a copy, the simulation adds listeners and plays
    #[arg(long, default_value = "data/play.db")]
    pub db: String,

    /// Listeners to create
    #[arg(long, default_value_t = 10, value_parser = at_least_one)]
    pub users: usize,

    /// Plays to store, one by one
    #[arg(long, default_value_t = 1000, value_parser = at_least_one)]
    pub events: usize,

    /// Seed of the random choices, the same seed repeats the run
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
}

#[derive(Debug, Args)]
pub struct BenchArgs {
    #[command(flatten)]
    pub data: DataArgs,

    /// Rankings to time
    #[arg(long, default_value_t = 200, value_parser = at_least_one)]
    pub requests: usize,

    /// Relaxed thresholds for tiny datasets such as sample.csv
    #[arg(long)]
    pub lenient: bool,

    /// Order of the Markov model, 1 or 2
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=2))]
    pub markov_order: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MigrateAction {
    /// List the migrations and whether they are applied
    Status,
    /// Apply the pending ones
    Up,
    /// Undo the last one (drops its tables and their data)
    Down,
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[arg(value_enum, default_value_t = MigrateAction::Up)]
    pub action: MigrateAction,

    #[arg(long, default_value = DEFAULT_DB)]
    pub db: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportKind {
    Users,
    Recommendations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportFormat {
    Csv,
    Json,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// What to write
    #[arg(value_enum)]
    pub what: ExportKind,

    #[arg(long, default_value = DEFAULT_DB)]
    pub db: String,

    #[arg(long, value_enum, default_value_t = ExportFormat::Csv)]
    pub format: ExportFormat,

    /// File to write; without it the rows go to the screen
    #[arg(long)]
    pub out: Option<String>,

    /// Recommendations per listener
    #[arg(long, default_value_t = 10, value_parser = at_least_one)]
    pub top: usize,

    /// Relaxed thresholds for tiny datasets such as sample.csv
    #[arg(long)]
    pub lenient: bool,
}

#[derive(Debug, Args)]
pub struct ResetArgs {
    #[arg(long, default_value = DEFAULT_DB)]
    pub db: String,

    /// Confirms that every row may be deleted
    #[arg(long)]
    pub yes: bool,
}

fn at_least_one(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(number) if number >= 1 => Ok(number),
        _ => Err(format!("`{value}` is not a whole number of at least 1")),
    }
}

/// More than 20 steps means thousands of combinations for no real gain.
fn steps_in_range(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(number) if (1..=20).contains(&number) => Ok(number),
        _ => Err(format!("`{value}` is not a whole number from 1 to 20")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("app").chain(args.iter().copied()))
    }

    #[test]
    fn definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_command_means_demo() {
        assert!(parse(&[]).unwrap().command.is_none());
    }

    #[test]
    fn recommend_needs_a_user_and_has_defaults() {
        assert!(parse(&["recommend"]).is_err());

        let Some(Command::Recommend(args)) = parse(&["recommend", "--user", "42"]).unwrap().command
        else {
            panic!("expected the recommend command");
        };
        assert_eq!(args.user, 42);
        assert_eq!(args.top, 10);
        assert_eq!(args.data.path, DEFAULT_DATA);
        assert!(!args.lenient);
    }

    #[test]
    fn recommend_accepts_path_and_options_in_any_order() {
        let cli = parse(&[
            "recommend",
            "-u",
            "7",
            "data/big.csv",
            "--top",
            "3",
            "--lenient",
        ]);
        let Some(Command::Recommend(args)) = cli.unwrap().command else {
            panic!("expected the recommend command");
        };
        assert_eq!((args.user, args.top), (7, 3));
        assert_eq!(args.data.path, "data/big.csv");
        assert!(args.lenient);
    }

    #[test]
    fn eval_reads_flags() {
        let cli = parse(&["eval", "data/x.csv", "--global", "-k", "5"]).unwrap();
        let Some(Command::Eval(args)) = cli.command else {
            panic!("expected the eval command");
        };
        assert!(args.global);
        assert_eq!(args.k, 5);
        assert_eq!(args.data.path, "data/x.csv");
    }

    #[test]
    fn old_style_eval_call_still_works() {
        assert!(parse(&["eval", "data/lastfm_valid.csv"]).is_ok());
    }

    #[test]
    fn tune_reuses_eval_options() {
        let cli = parse(&["tune", "--steps", "4", "--show", "2", "--global"]).unwrap();
        let Some(Command::Tune(args)) = cli.command else {
            panic!("expected the tune command");
        };
        assert_eq!((args.steps, args.show), (4, 2));
        assert!(args.eval.global);
    }

    #[test]
    fn nonsense_values_are_rejected() {
        assert!(parse(&["tune", "--steps", "0"]).is_err());
        assert!(parse(&["tune", "--steps", "21"]).is_err());
        assert!(parse(&["eval", "-k", "0"]).is_err());
        assert!(parse(&["recommend", "--user", "-1"]).is_err());
        assert!(parse(&["recommend", "--user", "1", "--top", "abc"]).is_err());
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert!(parse(&["dance"]).is_err());
    }

    #[test]
    fn etl_has_defaults() {
        let Some(Command::Etl(args)) = parse(&["etl"]).unwrap().command else {
            panic!("expected the etl command");
        };
        assert_eq!(args.db, DEFAULT_DB);
        assert_eq!(args.gap, 30);
    }

    #[test]
    fn serve_has_defaults() {
        let Some(Command::Serve(args)) = parse(&["serve"]).unwrap().command else {
            panic!("expected the serve command");
        };
        // nothing is decided here: the config file and the defaults fill the gaps later
        assert_eq!(args.db, None);
        assert_eq!(args.lenient, None);
        assert!(args.cors_origins.is_empty());
    }

    #[test]
    fn serve_flags_take_values() {
        let Some(Command::Serve(args)) =
            parse(&["serve", "--lenient", "--cors-origin", "http://a,http://b"])
                .unwrap()
                .command
        else {
            panic!("expected the serve command");
        };
        assert_eq!(args.lenient, Some(true));
        assert_eq!(args.cors_origins, ["http://a", "http://b"]);
    }
}
