#!/bin/sh
# Starts the API. Before the first start it loads MUSIC_CSV into MUSIC_DB,
# because `serve` needs a database made by `etl`. Any other command
# (eval, backtest, export, ...) runs the program with those arguments.
set -eu

app=music-recommendation-system

case "${1:-serve}" in
serve)
    if [ ! -f "$MUSIC_DB" ]; then
        echo "no database at $MUSIC_DB, loading $MUSIC_CSV"
        "$app" etl "$MUSIC_CSV" --db "$MUSIC_DB"
    fi
    [ "$#" -gt 0 ] && shift
    exec "$app" serve "$@"
    ;;
*)
    exec "$app" "$@"
    ;;
esac
