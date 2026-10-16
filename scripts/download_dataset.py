#!/usr/bin/env python3
"""Download the Last.fm 1K listening dataset and save it as CSV.

The data comes from the Hugging Face dataset matthewfranglen/lastfm-1k
(the `valid` part: 1.5M events, 99 users). The script downloads the
parquet file, then writes data/lastfm_valid.csv with the columns the ETL
expects:

    user_index, artist_index, track_index, timestamp,
    artist_name, track_name, gender, age, country

Usage:
    pip install huggingface_hub pandas pyarrow
    python scripts/download_dataset.py
    python scripts/download_dataset.py --out data/my.csv --force
    python scripts/download_dataset.py --parquet valid.gz.parquet   # already downloaded

Dataset: Celma, O. (2010). Music Recommendation and Discovery in the Long
Tail. Springer. Source: https://www.last.fm/
"""

import argparse
import sys
from pathlib import Path

REPO_ID = "matthewfranglen/lastfm-1k"
PARQUET_NAME = "valid.gz.parquet"
COLUMNS = [
    "user_index",
    "artist_index",
    "track_index",
    "timestamp",
    "artist_name",
    "track_name",
    "gender",
    "age",
    "country",
]


def download_parquet() -> Path:
    """Find valid.gz.parquet in the dataset repo and download it."""
    try:
        from huggingface_hub import hf_hub_download, list_repo_files
    except ImportError:
        sys.exit("Missing package. Run: pip install huggingface_hub pandas pyarrow")

    print(f"Looking for {PARQUET_NAME} in {REPO_ID} ...")
    files = list_repo_files(REPO_ID, repo_type="dataset")
    matches = [f for f in files if f.endswith(PARQUET_NAME)]
    if not matches:
        sys.exit(
            f"{PARQUET_NAME} was not found in {REPO_ID}. Files in the repo:\n  "
            + "\n  ".join(files)
            + "\nDownload the file by hand and pass it with --parquet."
        )
    remote = matches[0]
    print(f"Downloading {remote} (this can take a few minutes) ...")
    return Path(hf_hub_download(REPO_ID, remote, repo_type="dataset"))


def convert(parquet: Path, out: Path) -> int:
    """Write the parquet file to CSV in batches, so memory stays low."""
    try:
        import pandas as pd
        import pyarrow.parquet as pq
    except ImportError:
        sys.exit("Missing package. Run: pip install huggingface_hub pandas pyarrow")

    pf = pq.ParquetFile(parquet)
    present = pf.schema_arrow.names
    missing = [c for c in COLUMNS if c not in present]
    if missing:
        sys.exit(
            f"The parquet file has no columns {missing}.\nIt has: {present}"
        )

    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(out.suffix + ".part")
    rows = 0
    with open(tmp, "w", newline="", encoding="utf-8") as f:
        for i, batch in enumerate(pf.iter_batches(batch_size=200_000, columns=COLUMNS)):
            df = batch.to_pandas()[COLUMNS]
            # ETL expects ISO 8601 with a 'T' (2009-05-04T13:06:09) and an integer age.
            df["timestamp"] = pd.to_datetime(df["timestamp"]).dt.strftime("%Y-%m-%dT%H:%M:%S")
            df["age"] = df["age"].astype("Int64")
            df.to_csv(f, index=False, header=(i == 0), lineterminator="\n")
            rows += len(df)
            print(f"  {rows:,} rows written", end="\r")
    tmp.replace(out)
    print()
    return rows


def main() -> None:
    p = argparse.ArgumentParser(description="Download the Last.fm 1K dataset as CSV.")
    p.add_argument("--out", type=Path, default=Path("data/lastfm_valid.csv"),
                   help="output CSV (default: data/lastfm_valid.csv)")
    p.add_argument("--parquet", type=Path,
                   help="use this local parquet file instead of downloading")
    p.add_argument("--force", action="store_true", help="overwrite an existing CSV")
    args = p.parse_args()

    if args.out.exists() and not args.force:
        sys.exit(f"{args.out} already exists. Use --force to overwrite it.")

    parquet = args.parquet if args.parquet else download_parquet()
    rows = convert(parquet, args.out)
    size_mb = args.out.stat().st_size / 1_000_000
    print(f"Done: {rows:,} rows, {size_mb:.0f} MB -> {args.out}")
    print("Next: cargo run --release -- etl", args.out, "--db data/big.db")


if __name__ == "__main__":
    main()
