//! Likes and skips (R3): stored one row per feedback, summed per model for
//! the bandit with one GROUP BY.

use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, FromQueryResult, Set, Value};

use crate::db::entities::{feedback, track, user};
use crate::db::error::DbError;
use crate::db::sql::{bound, statement};
use crate::domain::{Feedback, FeedbackEntry, FeedbackKind, SourceFeedback, Track};

#[derive(Debug, FromQueryResult)]
struct EntryRow {
    track_index: u32,
    artist_index: u32,
    track_name: Option<String>,
    artist_name: Option<String>,
    kind: String,
    source: Option<String>,
    created_at: NaiveDateTime,
}

#[derive(Debug, FromQueryResult)]
struct StatsRow {
    source: String,
    likes: i64,
    skips: i64,
}

const PAGE: &str = "
    SELECT f.track_index, t.artist_index, t.track_name, t.artist_name, f.kind, f.source, f.created_at
    FROM feedback f
    JOIN tracks t ON t.track_index = f.track_index
    WHERE f.user_index = ?
    ORDER BY f.created_at DESC, f.id DESC
    LIMIT ?";

const STATS: &str = "
    SELECT source,
           SUM(CASE WHEN kind = 'like' THEN 1 ELSE 0 END) AS likes,
           SUM(CASE WHEN kind = 'skip' THEN 1 ELSE 0 END) AS skips
    FROM feedback
    WHERE user_index = ? AND source IS NOT NULL
    GROUP BY source
    ORDER BY source";

/// Stores one feedback.
///
/// # Errors
///
/// [`DbError::UserNotFound`] or [`DbError::TrackNotFound`] for unknown ids.
pub(crate) async fn insert_feedback(
    db: &DatabaseConnection,
    item: Feedback,
) -> Result<(), DbError> {
    if user::Entity::find_by_id(item.user_index)
        .one(db)
        .await?
        .is_none()
    {
        return Err(DbError::UserNotFound(item.user_index));
    }
    if track::Entity::find_by_id(item.track_index)
        .one(db)
        .await?
        .is_none()
    {
        return Err(DbError::TrackNotFound(item.track_index));
    }
    feedback::ActiveModel {
        user_index: Set(item.user_index),
        track_index: Set(item.track_index),
        kind: Set(item.kind.as_str().to_string()),
        source: Set(item.source),
        created_at: Set(item.created_at),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// The user's latest feedback with the tracks, newest first.
pub(crate) async fn feedback_page(
    db: &DatabaseConnection,
    user_index: u32,
    limit: usize,
) -> Result<Vec<FeedbackEntry>, DbError> {
    let rows =
        EntryRow::find_by_statement(statement(db, PAGE, [Value::from(user_index), bound(limit)]))
            .all(db)
            .await?;
    rows.into_iter()
        .map(|row| {
            let kind = FeedbackKind::parse(&row.kind)
                .ok_or_else(|| DbError::Storage(format!("unknown feedback kind {:?}", row.kind)))?;
            Ok(FeedbackEntry {
                feedback: Feedback {
                    user_index,
                    track_index: row.track_index,
                    kind,
                    source: row.source,
                    created_at: row.created_at,
                },
                track: Track {
                    track_index: row.track_index,
                    artist_index: row.artist_index,
                    track_name: row.track_name,
                    artist_name: row.artist_name,
                },
            })
        })
        .collect()
}

/// Likes and skips per model for one user; feedback without a model is left out.
pub(crate) async fn feedback_stats(
    db: &DatabaseConnection,
    user_index: u32,
) -> Result<Vec<SourceFeedback>, DbError> {
    let rows = StatsRow::find_by_statement(statement(db, STATS, [Value::from(user_index)]))
        .all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| SourceFeedback {
            source: row.source,
            likes: u32::try_from(row.likes).unwrap_or(u32::MAX),
            skips: u32::try_from(row.skips).unwrap_or(u32::MAX),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::db::error::DbError;
    use crate::db::store::Store;
    use crate::domain::test_utils::{event_of, named_track, user};
    use crate::domain::{Feedback, FeedbackKind};

    fn feedback(track: u32, kind: FeedbackKind, source: Option<&str>, minute: i64) -> Feedback {
        Feedback {
            user_index: 1,
            track_index: track,
            kind,
            source: source.map(str::to_string),
            created_at: event_of(1, track, minute).timestamp,
        }
    }

    async fn store() -> Store {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(1)]).await.unwrap();
        store
            .add_tracks(vec![
                named_track(10, 1, "Crocodile", "Underworld"),
                named_track(11, 1, "Rez", "Underworld"),
            ])
            .await
            .unwrap();
        store
    }

    #[tokio::test]
    async fn feedback_is_summed_per_model() {
        let store = store().await;
        for (minute, (kind, source)) in [
            (FeedbackKind::Like, Some("markov")),
            (FeedbackKind::Like, Some("markov")),
            (FeedbackKind::Skip, Some("markov")),
            (FeedbackKind::Skip, Some("cf")),
            (FeedbackKind::Like, None),
        ]
        .into_iter()
        .enumerate()
        {
            store
                .add_feedback(feedback(10, kind, source, minute as i64))
                .await
                .unwrap();
        }

        let stats = store.feedback_stats(1).await.unwrap();
        let summary: Vec<(&str, u32, u32)> = stats
            .iter()
            .map(|row| (row.source.as_str(), row.likes, row.skips))
            .collect();
        assert_eq!(summary, vec![("cf", 0, 1), ("markov", 2, 1)]);
    }

    #[tokio::test]
    async fn feedback_comes_back_newest_first_with_the_track() {
        let store = store().await;
        store
            .add_feedback(feedback(10, FeedbackKind::Like, None, 0))
            .await
            .unwrap();
        store
            .add_feedback(feedback(11, FeedbackKind::Skip, Some("cf"), 5))
            .await
            .unwrap();

        let page = store.feedback(1, 10).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].track.track_name.as_deref(), Some("Rez"));
        assert_eq!(page[0].feedback.kind, FeedbackKind::Skip);
        assert_eq!(store.feedback(1, 1).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unknown_users_and_tracks_are_rejected() {
        let store = store().await;
        let mut stranger = feedback(10, FeedbackKind::Like, None, 0);
        stranger.user_index = 9;
        assert_eq!(
            store.add_feedback(stranger).await,
            Err(DbError::UserNotFound(9))
        );
        assert_eq!(
            store
                .add_feedback(feedback(99, FeedbackKind::Like, None, 0))
                .await,
            Err(DbError::TrackNotFound(99))
        );
    }
}
