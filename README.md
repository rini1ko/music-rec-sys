# Music Recommendation System

A hybrid music recommender written in Rust. It learns from Last.fm listening
history and combines three models into one ranked list, with an explanation
for every recommended track.

## How it works

```
CSV -> ETL -> Repository -> [Trends | Markov | Co-listening] -> Ranker -> recommendations
```

| Part | What it does |
|---|---|
| ETL | Reads the CSV, skips and counts broken rows, splits listening events into sessions (a pause over 30 minutes starts a new one) |
| Repository | A trait with an in-memory implementation; writes are atomic and checked for referential integrity |
| Trends | What is popular right now (play counts in the latest time window) |
| Markov | First-order model: which track usually follows the last one in a session |
| Co-listening | Graph of tracks heard in the same sessions, weighted by Jaccard similarity, weak edges are pruned |
| Ranker | Normalizes each model's scores, applies weights, merges the votes, never recommends already heard tracks |
| Evaluation | Time-based train/test split, hit rate, precision, recall and NDCG, weight tuning with a holdout check |

## Usage

```bash
cargo run                                   # demo on data/sample.csv
cargo run -- --help                         # all commands

cargo run --release -- stats data/lastfm_valid.csv
cargo run --release -- recommend data/lastfm_valid.csv --user 123 --top 5
cargo run --release -- eval data/lastfm_valid.csv        # compare the models
cargo run --release -- eval data/lastfm_valid.csv --global -k 20
cargo run --release -- tune data/lastfm_valid.csv --steps 10
cargo test
```

| Command | What it does |
|---|---|
| `stats [PATH]` | loads a CSV and prints how many users, tracks, events and sessions it has |
| `recommend --user N [PATH]` | recommends tracks for one user with an explanation for each |
| `eval [PATH]` | compares every model and the hybrid on a time-based train/test split |
| `tune [PATH]` | searches for the best model weights and checks them on other users |
## Data

The project uses the Last.fm listening dataset collected by Òscar Celma,
in the version published on Hugging Face as
[matthewfranglen/lastfm-1k](https://huggingface.co/datasets/matthewfranglen/lastfm-1k).
The full dataset is **not** included in this repository, only a tiny
`data/sample.csv` for the demo and tests. Source: [Last.fm](https://www.last.fm/).

To prepare the data, download `valid.gz.parquet` from the dataset page and convert it to CSV
(columns: `user_index, artist_index, track_index, timestamp, artist_name, track_name, gender, age, country`).

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

## Results

Evaluated on the `valid` part of the dataset (1.5M listening events, 99 users).
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
- Adding collaborative filtering raised the hybrid NDCG from 0.0657 to 0.0896.
- Co-listening is the most precise single model (best NDCG). CF reaches the largest share of users (best hit rate) but ranks the relevant track lower. The models complement each other.
- Weight tuning picked weights on one half of the users and checked them on the other half.
  The tuned weights (mostly co-listening, a little CF) scored 0.0928 against 0.0875 for the defaults,
  a small gain that is within the noise for about 50 users per half, so the defaults are kept.

### Limitations

- With the per-user split, models can learn from other users' later listening,
  so the numbers are slightly optimistic. `--global` gives a stricter split.
- Only 99 users are evaluated, so small differences between models are noise.
- CF can only find neighbors among these 99 users; on the full dataset it would likely do better.
- Everything is stored in memory; a SQLite repository is planned.

### Limitations

- With the per-user split, models can learn from other users' later listening,
  so the numbers are slightly optimistic. `--global` gives a stricter split.
- Only 99 users are evaluated, so small differences between models are noise.
- Everything is stored in memory; a SQLite repository is planned.
