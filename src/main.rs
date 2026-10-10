mod cli;
mod commands;
mod logging;

use std::process::ExitCode;
use clap::Parser;

fn main() -> ExitCode {
    match commands::run(cli::Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // `{:#}` prints the whole chain: "failed to load x.csv: file not found"
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}