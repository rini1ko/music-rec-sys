//! A listener's history page by page (E3).

use chrono::NaiveDateTime;
use sea_orm::{DatabaseConnection, EntityTrait, FromQueryResult, Value};

use crate::db::entities::user;
use crate::db::error::DbError;
use crate::db::sql::{bound, count, statement};
use crate::domain::{HistoryEntry, HistoryPage, Track};

#[derive(Debug, FromQueryResult)]
struct TotalRow {
    total: i64,
}

#[derive(Debug, FromQueryResult)]
struct EntryRow {
    track_index: u32,
    artist_index: u32,
    track_name: Option<String>,
    artist_name: Option<String>,
    timestamp: NaiveDateTime,
}

const TOTAL: &str = "SELECT COUNT(*) AS total FROM listen_events WHERE user_index = ?";

// the (user_index, timestamp) index serves both the filter and the order
const PAGE: &str = "
    SELECT e.track_index, t.artist_index, t.track_name, t.artist_name, e.timestamp
    FROM listen_events e
    JOIN tracks t ON t.track_index = e.track_index
    WHERE e.user_index = ?
    ORDER BY e.timestamp DESC, e.id DESC
    LIMIT ? OFFSET ?";

/// Plays of a user, newest first.
///
/// # Errors
///
/// [`DbError::UserNotFound`] if there is no such user.
pub(crate) async fn history_page(
    db: &DatabaseConnection,
    user_index: u32,
    limit: usize,
    offset: usize,
) -> Result<HistoryPage, DbError> {
    if user::Entity::find_by_id(user_index)
        .one(db)
        .await?
        .is_none()
    {
        return Err(DbError::UserNotFound(user_index));
    }

    let user = Value::from(user_index);
    let total = TotalRow::find_by_statement(statement(db, TOTAL, [user.clone()]))
        .one(db)
        .await?
        .map_or(0, |row| count(row.total));
    let rows =
        EntryRow::find_by_statement(statement(db, PAGE, [user, bound(limit), bound(offset)]))
            .all(db)
            .await?;

    let items = rows
        .into_iter()
        .map(|row| HistoryEntry {
            track: Track {
                track_index: row.track_index,
                artist_index: row.artist_index,
                track_name: row.track_name,
                artist_name: row.artist_name,
            },
            timestamp: row.timestamp,
        })
        .collect();
    Ok(HistoryPage { total, items })
}

#[cfg(test)]
mod tests {
    use crate::db::error::DbError;
    use crate::db::store::Store;
    use crate::domain::test_utils::{event_of, named_track, user};

    async fn store() -> Store {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(1), user(2)]).await.unwrap();
        store
            .add_tracks(vec![
                named_track(10, 100, "Crocodile", "Underworld"),
                named_track(11, 100, "Born Slippy", "Underworld"),
            ])
            .await
            .unwrap();
        store
            .add_events(vec![
                event_of(1, 10, 0),
                event_of(1, 11, 1),
                event_of(1, 10, 2),
                event_of(2, 11, 0),
            ])
            .await
            .unwrap();
        store
    }

    #[tokio::test]
    async fn history_is_paged_newest_first_with_names() {
        let store = store().await;

        let first = store.history_page(1, 2, 0).await.unwrap();
        assert_eq!(first.total, 3);
        let tracks: Vec<u32> = first
            .items
            .iter()
            .map(|item| item.track.track_index)
            .collect();
        assert_eq!(tracks, vec![10, 11]);
        assert_eq!(
            first.items[1].track.track_name.as_deref(),
            Some("Born Slippy")
        );
        assert!(first.items[0].timestamp > first.items[1].timestamp);

        let rest = store.history_page(1, 2, 2).await.unwrap();
        assert_eq!(rest.items.len(), 1);
        assert!(store.history_page(1, 2, 10).await.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn history_of_an_unknown_user_is_an_error() {
        let result = store().await.history_page(9, 10, 0).await;
        assert_eq!(result, Err(DbError::UserNotFound(9)));
    }

    #[tokio::test]
    async fn several_tracks_are_read_in_one_go() {
        let tracks = store().await.tracks(&[11, 10, 99]).await.unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[&11].track_name.as_deref(), Some("Born Slippy"));
    }
}
