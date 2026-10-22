//! Artists (E2). There is no artists table: an artist is the set of tracks
//! with the same `artist_index`, so the queries group the tracks table.

use sea_orm::{DatabaseConnection, FromQueryResult, Value};

use crate::db::error::DbError;
use crate::db::sql::{ArtistRow, TrackPlaysRow, bound, contains_pattern, statement};
use crate::domain::{ArtistProfile, ArtistSummary};

const SEARCH: &str = r"
    SELECT t.artist_index, MAX(t.artist_name) AS artist_name,
           COUNT(DISTINCT t.track_index) AS tracks,
           COUNT(e.id) AS plays,
           COUNT(DISTINCT e.user_index) AS listeners
    FROM tracks t
    LEFT JOIN listen_events e ON e.track_index = t.track_index
    WHERE t.artist_name LIKE ? ESCAPE '\'
    GROUP BY t.artist_index
    ORDER BY plays DESC, t.artist_index
    LIMIT ?";

const SUMMARY: &str = "
    SELECT t.artist_index, MAX(t.artist_name) AS artist_name,
           COUNT(DISTINCT t.track_index) AS tracks,
           COUNT(e.id) AS plays,
           COUNT(DISTINCT e.user_index) AS listeners
    FROM tracks t
    LEFT JOIN listen_events e ON e.track_index = t.track_index
    WHERE t.artist_index = ?
    GROUP BY t.artist_index";

const TOP_TRACKS: &str = "
    SELECT t.track_index, t.artist_index, t.track_name, t.artist_name, COUNT(e.id) AS plays
    FROM tracks t
    LEFT JOIN listen_events e ON e.track_index = t.track_index
    WHERE t.artist_index = ?
    GROUP BY t.track_index
    ORDER BY plays DESC, t.track_index
    LIMIT ?";

/// Artists whose name contains `text`, the most played first.
pub(crate) async fn search_artists(
    db: &DatabaseConnection,
    text: &str,
    limit: usize,
) -> Result<Vec<ArtistSummary>, DbError> {
    let pattern = Value::from(contains_pattern(text));
    let rows = ArtistRow::find_by_statement(statement(db, SEARCH, [pattern, bound(limit)]))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// The artist with their `top` most played tracks, `None` if no track has this artist.
pub(crate) async fn artist_profile(
    db: &DatabaseConnection,
    artist_index: u32,
    top: usize,
) -> Result<Option<ArtistProfile>, DbError> {
    let artist = Value::from(artist_index);
    let Some(summary) = ArtistRow::find_by_statement(statement(db, SUMMARY, [artist.clone()]))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    let tracks = TrackPlaysRow::find_by_statement(statement(db, TOP_TRACKS, [artist, bound(top)]))
        .all(db)
        .await?;

    Ok(Some(ArtistProfile {
        summary: summary.into(),
        top_tracks: tracks.into_iter().map(Into::into).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use crate::db::store::Store;
    use crate::domain::test_utils::{event_of, named_track, user};

    async fn store() -> Store {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(1), user(2)]).await.unwrap();
        store
            .add_tracks(vec![
                named_track(10, 100, "Crocodile", "Underworld"),
                named_track(11, 100, "Born Slippy", "Underworld"),
                named_track(12, 100, "Rez", "Underworld"),
                named_track(20, 200, "Kids", "MGMT"),
                named_track(30, 300, "Song", "100%_Pure"),
            ])
            .await
            .unwrap();
        store
            .add_events(vec![
                event_of(1, 11, 0),
                event_of(2, 11, 1),
                event_of(1, 10, 2),
                event_of(1, 20, 3),
            ])
            .await
            .unwrap();
        store
    }

    #[tokio::test]
    async fn artists_are_found_by_a_part_of_the_name() {
        let store = store().await;

        let found = store.search_artists("world", 10).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist_index, 100);
        assert_eq!(
            (found[0].tracks, found[0].plays, found[0].listeners),
            (3, 3, 2)
        );

        assert!(store.search_artists("zzz", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn like_wildcards_are_matched_literally() {
        let store = store().await;
        // without escaping, "%_" would match every name
        let found = store.search_artists("0%_", 10).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist_index, 300);
    }

    #[tokio::test]
    async fn artist_page_lists_tracks_by_plays_including_unplayed_ones() {
        let profile = store().await.artist(100, 10).await.unwrap().unwrap();

        assert_eq!(profile.summary.artist_name.as_deref(), Some("Underworld"));
        let tracks: Vec<(u32, usize)> = profile
            .top_tracks
            .iter()
            .map(|item| (item.track.track_index, item.plays))
            .collect();
        assert_eq!(tracks, vec![(11, 2), (10, 1), (12, 0)]);
    }

    #[tokio::test]
    async fn unknown_artist_is_none() {
        assert_eq!(store().await.artist(999, 10).await.unwrap(), None);
    }
}
