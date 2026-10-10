use clap::{Args, Parser, Subcommand};

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
    /// Load a CSV file into the SQLite database
    Etl(EtlArgs),
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

    /// Weights are tried in steps of 1/STEPS (10 means 286 combinations)
    #[arg(long, default_value_t = 10, value_parser = steps_in_range)]
    pub steps: usize,

    /// How many of the best combinations to print
    #[arg(long, default_value_t = 5, value_parser = at_least_one)]
    pub show: usize,
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
        let cli = parse(&["recommend", "-u", "7", "data/big.csv", "--top", "3", "--lenient"]);
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
}