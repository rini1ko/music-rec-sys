# Music Recommendation System

A hybrid music recommender written in Rust, with a small React front end.
It learns from Last.fm listening history, combines five models with a set
of rules into one ranked list and explains every recommended track
("people with similar taste listened to this", "often played right after
your last track", ...). It runs as a command-line tool and as an HTTP API.

## Features

**Recommendations**
- Five models, weighted and merged by a ranker: trends, a Markov chain
  (first or second order), a co-listening graph (Jaccard similarity),
  user-based collaborative filtering and implicit-feedback matrix
  factorization (ALS, solved with its own Cholesky routine).
- Rules on top: cold start for listeners without plays (popular in their
  country, then overall), a bonus for the listener's favourite artist and at
  most three tracks per artist.
- An explanation for every track: which models voted for it and how much.
- Model weights can be changed for a single request.
- Online learning: a new play updates the next-track and trend models at
  once, without retraining.
- Likes and skips per track, and Thompson sampling that leans each listener's
  weights towards the models they like.
- "Similar tracks" for any track and a radio playlist that keeps going from
  each picked track.

**Evaluation**
- Train/test split per user or by one global time cutoff.
- Hit rate, precision, recall and NDCG, plus catalog coverage, novelty and
  artist diversity.
- Every model alone, the hybrid, and the hybrid with its rules (including
  listeners without history) side by side.
- Weight tuning on a grid, picked on one half of the users and checked on
  the other half.

**Data and operations**
- ETL from CSV into SQLite with progress, broken-row reporting and session
  detection; `--append` adds new files without duplicates.
- Versioned schema migrations.
- HTTP API (axum) with Swagger UI, statistics for a dashboard, artists,
  paged listening history, a Server-Sent Events stream of plays,
  background retraining and evaluation.
- Prometheus metrics, an LRU cache of recommendation lists, rate limiting,
  request ids, an API key for writes, body size and time limits, JSON errors
  for every failure.
- Configuration from the command line, `MUSIC_*` environment variables and
  `config.toml`.
- Load simulation, benchmarks of training and ranking, CSV/JSON export.

**Front end** (`frontend/`): pick a listener, see recommendations with their
reasons, play tracks, compare your own model weights with the defaults,
create a listener and watch the cold start, walk chains of similar tracks,
and a dashboard with model quality and activity charts.

## Installation and build

You need Rust 1.88 or newer (edition 2024) and, for the front end, Node.js 22
or newer. On Windows the MSVC toolchain also needs Visual Studio Build Tools
with the C++ workload.

```bash
git clone https://github.com/rini1ko/music-recommendation-system.git
cd music-recommendation-system
cargo build --release

# front end
cd frontend
npm ci
npm run build
```

The first build downloads the Swagger UI files, so it needs internet access.

## Data

The project uses the Last.fm listening dataset collected by Òscar Celma,
in the version published on Hugging Face as
[matthewfranglen/lastfm-1k](https://huggingface.co/datasets/matthewfranglen/lastfm-1k).
The full dataset is **not** included in this repository, only a tiny
`data/sample.csv` for the demo and tests. Source: [Last.fm](https://www.last.fm/).

To prepare the data, download `valid.gz.parquet` from the dataset page and convert it to CSV
(columns: `user_index, artist_index, track_index, timestamp, artist_name, track_name, gender, age, country`).
The examples below use `data/lastfm_valid.csv` for that file.

### Citation

When using this dataset you must reference the Last.fm webpage and cite:

```bibtex
@book{Celma:Springer2010,
    author = {Celma, O.},
    title = {{Music Recommendation and Discovery in the Long Tail}},
    publisher = {Springer},
    year = {2010}
}
```

## Usage

### Command line

```bash
cargo run                                          # demo on data/sample.csv
cargo run -- --help                                # every command

# work on a CSV directly, in memory
cargo run --release -- stats data/lastfm_valid.csv
cargo run --release -- recommend data/lastfm_valid.csv --user 123 --top 5
cargo run --release -- eval data/lastfm_valid.csv
cargo run --release -- eval data/lastfm_valid.csv --global -k 20
cargo run --release -- tune data/lastfm_valid.csv --steps 10

# load into SQLite once, then work on the database
cargo run --release -- etl data/lastfm_valid.csv --db data/big.db
cargo run --release -- etl data/more_plays.csv --db data/big.db --append
cargo run --release -- eval --db data/big.db
cargo run --release -- migrate status --db data/big.db
cargo run --release -- export recommendations --db data/big.db --format csv --out recs.csv
cargo run --release -- export users --db data/big.db --format json

# measurements (simulate writes to the database, use a copy)
cargo run --release -- bench --db data/big.db --requests 200 --markov-order 2
cp data/big.db data/play.db
cargo run --release -- simulate --db data/play.db --users 10 --events 1000

cargo run --release -- reset-db --db data/play.db --yes
```

| Command | What it does |
|---|---|
| `stats` | users, tracks, events and sessions of a CSV or a database |
| `recommend --user N` | recommendations for one user with an explanation for each |
| `eval` | every model, the hybrid and the hybrid with rules on a train/test split |
| `tune` | searches for the best model weights and checks them on other users |
| `etl [--append]` | loads a CSV into SQLite; `--append` skips plays that are already stored |
| `serve` | starts the HTTP API |
| `migrate [status\|up\|down]` | shows or applies the schema migrations |
| `export users\|recommendations` | writes listeners or recommendations as CSV or JSON |
| `bench` | time to train every model and to rank |
| `simulate` | creates listeners and plays through the database layer and prints latency percentiles |
| `reset-db --yes` | deletes every row and recreates the schema |

### HTTP API

```bash
cargo run --release -- etl data/sample.csv --db data/demo.db
cargo run --release -- serve --db data/demo.db --lenient
```

Swagger UI is at <http://127.0.0.1:3000/docs/>, the OpenAPI document at
`/api-docs/openapi.json`. A few requests:

```bash
curl "http://127.0.0.1:3000/recommendations?user=1&top=5"
curl "http://127.0.0.1:3000/recommendations?user=1&markov=0&mf=1"      # weights for this request
curl "http://127.0.0.1:3000/recommendations/compare?user=1&trends=1"    # default list vs. these weights
curl "http://127.0.0.1:3000/tracks/929335/similar?top=5"
curl "http://127.0.0.1:3000/users/1/radio?length=10"
curl "http://127.0.0.1:3000/stats/overview?days=30"
curl -X POST -H "content-type: application/json" \
     -d '{"user":1,"track":500100}' http://127.0.0.1:3000/events
curl -X POST -H "content-type: application/json" \
     -d '{"user":1,"track":929335,"kind":"like"}' http://127.0.0.1:3000/feedback
curl -X POST http://127.0.0.1:3000/model/evaluate
curl http://127.0.0.1:3000/model/evaluation
curl -N http://127.0.0.1:3000/events/stream                            # Server-Sent Events
curl http://127.0.0.1:3000/metrics                                     # Prometheus
```

On Windows PowerShell use `curl.exe` or `Invoke-RestMethod`. When the server
has an API key, writes (`POST /events`, `/users`, `/feedback`, `/model/*`)
need the header `x-api-key`. Every error is JSON: `{"error": "..."}`.

### Front end

```bash
cd frontend
npm run dev            # http://localhost:5173, talks to http://127.0.0.1:3000
```

Set `VITE_API_URL` to use another API address and `VITE_API_KEY` when the
server requires a key.

## Configuration

`serve` reads its settings from three places. The command line wins over
`MUSIC_*` environment variables, which win over `config.toml`, which wins
over the defaults. Copy `config.example.toml` to `config.toml` and change
what you need; unknown keys are reported as errors. `config.toml` is in
`.gitignore` because it may hold the API key.

```toml
[server]
db = "data/big.db"
addr = "127.0.0.1:3000"
api_key = "change-me"        # leave out to keep writes open (local development)
rate_per_second = 20         # per client, 0 turns the limit off
request_timeout_secs = 30
cache_capacity = 1024        # 0 turns the cache off

[engine]
lenient = false              # low thresholds for tiny datasets
markov_order = 2             # 1 or 2
session_gap_minutes = 30

[engine.weights]
trends = 0.1
markov = 0.2
co_listening = 0.4
cf = 0.3
mf = 0.0                     # matrix factorization is off unless given a weight
```

Every key has a flag and a variable, for example `--api-key` / `MUSIC_API_KEY`
or `--rate-per-second` / `MUSIC_RATE_PER_SECOND`; `cargo run -- serve --help`
lists them all. The log level follows `RUST_LOG` (for example
`RUST_LOG=debug`); logs go to the console and to JSON files in `logs/`.

## Project structure

```
src/
  main.rs, cli.rs        command-line entry point and argument definitions
  commands.rs, tools.rs  the commands: demo, stats, recommend, eval, tune, etl, serve,
                         simulate, bench, migrate, export, reset-db
  logging.rs             console and JSON-file logging
  domain.rs              users, tracks, plays, sessions, feedback and the result types
  etl/                   CSV rows, loading, session detection, appending without duplicates
  db/                    the Repository trait (sync, for the CLI) with in-memory and SQLite
                         implementations, the async Store for the API, sea-orm entities,
                         migrations, and SQL for statistics, artists, history and feedback
  models/                trends, Markov chain, co-listening graph, collaborative filtering,
                         matrix factorization, online counts, the bandit, the catalog
                         behind the rules, and adapters that turn models into candidate generators
  ranker.rs              normalizes, weights and merges the candidates of every model
  recommender.rs         the ranker plus cold start, artist bonus, variety and the radio
  engine.rs              weights and configuration, trains every model and wires the ranker
  evaluation/            split, accuracy and beyond-accuracy metrics, model comparison,
                         evaluation of the rules, weight tuning
  api/                   router and middleware (request ids, rate limit, API key, time limit,
                         metrics), handlers, DTOs, errors, OpenAPI document, cache, SSE stream
  config.rs, config/     server settings and config.toml
  cache.rs               a small LRU cache
  bench.rs               latency percentiles, load simulation, training benchmark
  export.rs              CSV and JSON export
  rng.rs                 random numbers for the bandit and the simulation
tests/
  common/mod.rs          a small library of listeners, tracks and plays shared by the tests
  api.rs                 request flows through the router: plays, retraining, likes, keys, errors
  pipeline.rs            CSV -> loader -> models -> evaluation and weight search, without HTTP
  cli.rs                 the built program: exit codes and printed output of every command
frontend/                React + TypeScript + Tailwind front end (Vite)
data/sample.csv          twelve rows for the demo and the tests (one is broken on purpose)
config.example.toml      every configuration key with its default
```

## Dependencies

| Crate | Why |
|---|---|
| `anyhow` | error context in the command-line binary |
| `axum`, `tower-http` | HTTP server, CORS and request tracing |
| `chrono` | timestamps and time windows |
| `clap` (`derive`, `env`) | command line, with `MUSIC_*` environment variables |
| `csv` | reading the dataset and writing exports |
| `futures-util` | turning the play channel into the Server-Sent Events stream |
| `indicatif` | progress bar of the ETL |
| `petgraph` | the co-listening graph |
| `sea-orm` | SQLite access |
| `sea-orm-migration` | versioned schema migrations |
| `serde`, `serde_json` | JSON for the API and exports |
| `thiserror` | typed errors in the library |
| `tokio` | async runtime, background training, broadcast channel |
| `toml` | reading `config.toml` |
| `tracing`, `tracing-subscriber`, `tracing-appender` | structured logs to the console and JSON files |
| `utoipa`, `utoipa-swagger-ui` | the OpenAPI document and Swagger UI |
| `tower` (dev) | calling the router in tests |

## Running the tests

```bash
cargo test                                  # unit and integration tests, no network or big files needed
cargo test --test api                       # one integration file: api, pipeline or cli
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

cd frontend
npm run lint
npm run build
```

There are two kinds of tests. Unit tests sit next to the code in
`#[cfg(test)] mod tests` blocks and check single functions, private ones
included (271 tests). Integration tests in `tests/` use only the public API of
the library or the built binary (26 tests). The API tests run the real router
on an in-memory SQLite database, so they need neither a server nor the full
dataset. The CLI tests write their databases under `target/`.

## Results

Evaluated on the `valid` part of the dataset (1.5M listening events, 99 users)
before matrix factorization and the rules were added to the comparison.
For every user the newest 20% of their history is held out and the models
try to guess what they listened to later. Only tracks the user had not heard before count.

| Model | hit@10 | NDCG@10 |
|---|---|---|
| Trends only | 0.071 | 0.0080 |
| Markov only | 0.101 | 0.0168 |
| Co-listening only | 0.242 | 0.0726 |
| Collaborative filtering only | 0.303 | 0.0555 |
| Hybrid (0.1 / 0.2 / 0.4 / 0.3) | **0.333** | **0.0896** |

- The hybrid gives at least one relevant track in the top 10 to 33% of users and is about 11 times better than the popularity baseline by NDCG.
- Co-listening is the most precise single model (best NDCG). CF reaches the largest share of users (best hit rate) but ranks the relevant track lower. The models complement each other.
- Weight tuning picked weights on one half of the users and checked them on the other half.
  The tuned weights scored 0.0928 against 0.0875 for the defaults, a small gain within the noise
  for about 50 users per half, so the defaults are kept.

Run `cargo run --release -- eval --db data/big.db` for the current numbers of every row,
including matrix factorization, the second-order Markov chain and the hybrid with rules.

### Limitations

- With the per-user split, models can learn from other users' later listening,
  so the numbers are slightly optimistic. `--global` gives a stricter split.
- Only 99 users are evaluated, so small differences between models are noise.
- CF and the bandit only know these 99 users; on the full dataset they would likely do better.

## License

No license has been chosen for the code yet, so all rights are reserved by
the author until a `LICENSE` file is added. The Last.fm dataset keeps its own
terms of use (see [Data](#data)).
