//! Helpers for the hand-written aggregation queries (statistics, artists,
//! history). They use plain SQL with bound parameters: GROUP BY over joins and
//! SQLite date functions read more clearly this way than through the builder.

use sea_orm::{DatabaseConnection, FromQueryResult, Statement, Value};

use crate::domain::{ArtistSummary, Track, TrackPlays};

pub(crate) fn statement(
    db: &DatabaseConnection,
    sql: &str,
    values: impl IntoIterator<Item = Value>,
) -> Statement {
    Statement::from_sql_and_values(db.get_database_backend(), sql, values)
}

/// SQLite returns counts as i64. They are never negative.
pub(crate) fn count(value: i64) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// `LIMIT` and `OFFSET` are bound as i64.
pub(crate) fn bound(value: usize) -> Value {
    Value::from(i64::try_from(value).unwrap_or(i64::MAX))
}

/// A `LIKE` pattern that matches `text` anywhere. `%`, `_` and `\` in the text
/// are matched literally; the query must say `ESCAPE '\'`.
pub(crate) fn contains_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for character in text.chars() {
        if matches!(character, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
}

/// A row of `track_index, artist_index, track_name, artist_name, plays`.
#[derive(Debug, FromQueryResult)]
pub(crate) struct TrackPlaysRow {
    pub track_index: u32,
    pub artist_index: u32,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub plays: i64,
}

impl From<TrackPlaysRow> for TrackPlays {
    fn from(row: TrackPlaysRow) -> Self {
        TrackPlays {
            track: Track {
                track_index: row.track_index,
                artist_index: row.artist_index,
                track_name: row.track_name,
                artist_name: row.artist_name,
            },
            plays: count(row.plays),
        }
    }
}

/// A row of `artist_index, artist_name, tracks, plays, listeners`.
#[derive(Debug, FromQueryResult)]
pub(crate) struct ArtistRow {
    pub artist_index: u32,
    pub artist_name: Option<String>,
    pub tracks: i64,
    pub plays: i64,
    pub listeners: i64,
}

impl From<ArtistRow> for ArtistSummary {
    fn from(row: ArtistRow) -> Self {
        Self {
            artist_index: row.artist_index,
            artist_name: row.artist_name,
            tracks: count(row.tracks),
            plays: count(row.plays),
            listeners: count(row.listeners),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_wildcards_in_the_text_are_escaped() {
        assert_eq!(contains_pattern("ab"), "%ab%");
        assert_eq!(contains_pattern("100%_x\\"), "%100\\%\\_x\\\\%");
    }

    #[test]
    fn negative_counts_become_zero() {
        assert_eq!(count(-1), 0);
        assert_eq!(count(7), 7);
    }
}
