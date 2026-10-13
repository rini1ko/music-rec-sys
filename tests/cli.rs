//! The built program, run the way a person runs it: arguments in, exit code
//! and printed text out. Each test works on its own database under `target/`.

use std::path::Path;
use std::process::{Command, Output};

const SAMPLE: &str = "data/sample.csv";

/// A database file named after the test, removed before and after it.
struct TestDb(String);

impl TestDb {
    fn new(name: &str) -> Self {
        let path = format!("target/cli-it-{name}-{}.db", std::process::id());
        let _ = std::fs::remove_file(&path);
        TestDb(path)
    }

    fn loaded(name: &str) -> Self {
        let db = TestDb::new(name);
        let out = run(&["etl", SAMPLE, "--db", &db.0]);
        assert!(out.status.success(), "{}", stderr(&out));
        db
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // on Windows the file can stay locked for a moment, a leftover is harmless
        let _ = std::fs::remove_file(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_music-recommendation-system"))
        .args(args)
        .env("RUST_LOG", "off")
        .output()
        .expect("the program starts")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `name: 12` out of a line such as `users: 3, tracks: 9, events: 11`.
fn count(text: &str, name: &str) -> usize {
    let start = text
        .find(&format!("{name}: "))
        .unwrap_or_else(|| panic!("no {name} in {text}"));
    text[start + name.len() + 2..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

#[test]
fn help_lists_every_command() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for command in [
        "stats",
        "recommend",
        "eval",
        "tune",
        "backtest",
        "etl",
        "serve",
        "simulate",
        "bench",
        "migrate",
        "export",
        "reset-db",
    ] {
        assert!(text.contains(command), "{command} is missing from:\n{text}");
    }
}

#[test]
fn wrong_arguments_fail_before_any_work() {
    let missing_user = run(&["recommend", SAMPLE]);
    assert_eq!(missing_user.status.code(), Some(2));
    assert!(stderr(&missing_user).contains("--user"));

    let zero = run(&["recommend", SAMPLE, "--user", "2", "--top", "0"]);
    assert_eq!(zero.status.code(), Some(2));
    assert!(stderr(&zero).contains("at least 1"));

    let steps = run(&["tune", SAMPLE, "--steps", "50"]);
    assert_eq!(steps.status.code(), Some(2));
    assert!(stderr(&steps).contains("from 1 to 20"));
}

#[test]
fn errors_name_the_missing_file_and_exit_with_one() {
    let out = run(&["stats", "data/no-such-file.csv"]);
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.starts_with("error: "), "{text}");
    assert!(text.contains("no-such-file.csv"), "{text}");

    let out = run(&["export", "users", "--db", "target/cli-it-no-such.db"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        !Path::new("target/cli-it-no-such.db").exists(),
        "export created a database"
    );
}

#[test]
fn the_csv_and_the_database_give_the_same_numbers() {
    let from_csv = run(&["stats", SAMPLE]);
    assert!(from_csv.status.success(), "{}", stderr(&from_csv));

    let db = TestDb::loaded("stats");
    let from_db = run(&["stats", "--db", &db.0]);
    assert!(from_db.status.success(), "{}", stderr(&from_db));

    for name in ["users", "tracks", "events", "sessions"] {
        assert_eq!(
            count(&stdout(&from_csv), name),
            count(&stdout(&from_db), name),
            "{name}"
        );
    }
}

#[test]
fn a_loaded_database_refuses_a_second_load_but_takes_an_append() {
    let db = TestDb::loaded("etl");
    let again = run(&["etl", SAMPLE, "--db", &db.0]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("already contains data"));

    let append = run(&["etl", SAMPLE, "--db", &db.0, "--append"]);
    assert!(append.status.success(), "{}", stderr(&append));
    let stats = stdout(&run(&["stats", "--db", &db.0]));
    assert_eq!(
        count(&stats, "events"),
        count(&stdout(&run(&["stats", SAMPLE])), "events")
    );
}

#[test]
fn recommend_prints_ranked_tracks_for_a_known_user() {
    let out = run(&[
        "recommend",
        SAMPLE,
        "--user",
        "2",
        "--top",
        "3",
        "--lenient",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!stdout(&out).trim().is_empty());

    let unknown = run(&["recommend", SAMPLE, "--user", "999999"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(stderr(&unknown).contains("999999"));
}

#[test]
fn export_writes_json_for_every_listener() {
    let db = TestDb::loaded("export");
    let users = count(&stdout(&run(&["stats", "--db", &db.0])), "users");

    let out = run(&["export", "users", "--db", &db.0, "--format", "json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON on stdout");
    assert_eq!(rows.as_array().map(Vec::len), Some(users));

    let file = format!("{}.csv", db.0);
    let out = run(&[
        "export",
        "recommendations",
        "--db",
        &db.0,
        "--out",
        &file,
        "--top",
        "2",
        "--lenient",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("rows written to"));
    let text = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(text.starts_with("user_index,rank,track_index"), "{text}");
}

#[test]
fn migrations_are_applied_and_reset_needs_yes() {
    let db = TestDb::loaded("migrate");
    let status = run(&["migrate", "status", "--db", &db.0]);
    assert!(status.status.success(), "{}", stderr(&status));
    let lines: Vec<String> = stdout(&status).lines().map(str::to_string).collect();
    assert!(lines.len() >= 2, "{lines:?}");
    assert!(
        lines.iter().all(|line| line.starts_with("applied ")),
        "{lines:?}"
    );

    let refused = run(&["reset-db", "--db", &db.0]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(stderr(&refused).contains("--yes"));
    assert!(count(&stdout(&run(&["stats", "--db", &db.0])), "events") > 0);

    let reset = run(&["reset-db", "--db", &db.0, "--yes"]);
    assert!(reset.status.success(), "{}", stderr(&reset));
    assert_eq!(count(&stdout(&run(&["stats", "--db", &db.0])), "events"), 0);
}

#[test]
fn backtest_judges_each_window_and_sums_them_up() {
    let out = run(&[
        "backtest",
        SAMPLE,
        "--windows",
        "2",
        "--days",
        "1",
        "-k",
        "5",
        "--lenient",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("window 1: ") && text.contains("window 2: "),
        "{text}"
    );
    // the sample spans two days, so the older window has nothing to train on
    assert!(text.contains("not judged"), "{text}");
    assert!(text.contains("best: "), "{text}");
    assert!(
        text.contains("mean ndcg") && text.contains("hybrid_rules"),
        "{text}"
    );

    let none = run(&["backtest", SAMPLE, "--windows", "0"]);
    assert_eq!(none.status.code(), Some(2));
    // too many days for a duration, and a duration that reaches before any date
    for (days, message) in [("999999999999", "too large"), ("99999999999", "reach back")] {
        let out = run(&["backtest", SAMPLE, "--days", days]);
        assert_eq!(out.status.code(), Some(1), "{days}");
        assert!(stderr(&out).contains(message), "{}", stderr(&out));
    }
}
