use chrono::Duration;
use sea_orm::DatabaseConnection;

use crate::db::error::DbError;
use crate::db::repository::Counts;
use crate::db::schema::{connect, create_schema};
use crate::db::sea_repository as queries;
use crate::domain::{ListenEvent, Session, Track, User};

/// The same database as `SeaOrmRepository`, but async, for the web server.
/// Cloning is cheap: all clones share one connection pool.
#[derive(Clone)]
pub struct Store {
    db: DatabaseConnection,
}

impl Store {
    /// `url` is e.g. `sqlite://data/music.db?mode=rwc`. Creates the tables if needed.
    pub async fn open(url: &str) -> Result<Self, DbError> {
        let db = connect(url).await?;
        create_schema(&db).await?;
        Ok(Self { db })
    }

    pub async fn add_users(&self, users: Vec<User>) -> Result<(), DbError> {
        queries::insert_users(&self.db, users).await
    }

    pub async fn add_tracks(&self, tracks: Vec<Track>) -> Result<(), DbError> {
        queries::insert_tracks(&self.db, tracks).await
    }

    pub async fn add_events(&self, events: Vec<ListenEvent>) -> Result<(), DbError> {
        queries::insert_events(&self.db, events).await
    }
    
    pub async fn add_sessions(&self, sessions: Vec<Session>) -> Result<(), DbError> {
        queries::insert_sessions(&self.db, sessions).await
    }
    
    pub async fn all_events(&self) -> Result<Vec<ListenEvent>, DbError> {
        queries::all_events(&self.db).await
    }

    pub async fn all_sessions(&self) -> Result<Vec<Session>, DbError> {
        queries::all_sessions(&self.db).await
    }

    pub async fn user(&self, user_index: u32) -> Result<Option<User>, DbError> {
        queries::find_user(&self.db, user_index).await
    }

    pub async fn track(&self, track_index: u32) -> Result<Option<Track>, DbError> {
        queries::find_track(&self.db, track_index).await
    }

    pub async fn history(&self, user_index: u32) -> Result<Vec<u32>, DbError> {
        queries::history(&self.db, user_index).await
    }

    pub async fn trending(
        &self,
        window: Duration,
        limit: usize,
    ) -> Result<Vec<(u32, usize)>, DbError> {
        queries::trending(&self.db, window, limit).await
    }

    pub async fn counts(&self) -> Result<Counts, DbError> {
        queries::count_rows(&self.db).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, track, user};

    #[tokio::test]
    async fn events_are_stored_and_read_back() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_users(vec![user(1)]).await.unwrap();
        store.add_tracks(vec![track(10), track(11)]).await.unwrap();
        store
            .add_events(vec![event_of(1, 11, 5), event_of(1, 10, 0)])
            .await
            .unwrap();

        assert_eq!(store.history(1).await, Ok(vec![10, 11]));
        assert_eq!(store.counts().await.unwrap().events, 2);
    }

    #[tokio::test]
    async fn events_of_unknown_users_are_rejected() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store.add_tracks(vec![track(10)]).await.unwrap();

        let result = store.add_events(vec![event_of(2, 10, 0)]).await;
        assert_eq!(result, Err(DbError::UserNotFound(2)));
    }
}