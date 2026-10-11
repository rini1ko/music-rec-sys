use chrono::{Duration, NaiveDateTime};
use sea_orm::DatabaseConnection;

use crate::db::error::DbError;
use crate::db::repository::Counts;
use crate::db::schema::{connect, create_schema};
use crate::db::sea_repository as queries;
use crate::domain::{ListenEvent, Session, Track, User, UsersPage};

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

    pub async fn latest_event_time(&self) -> Result<Option<NaiveDateTime>, DbError> {
        queries::latest_event_time(&self.db).await
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

    pub async fn users_page(&self, limit: usize, offset: usize) -> Result<UsersPage, DbError> {
        queries::users_page(&self.db, limit, offset).await
    }

    pub async fn search_tracks(&self, text: &str, limit: usize) -> Result<Vec<Track>, DbError> {
        queries::search_tracks(&self.db, text, limit).await
    }

    pub async fn all_tracks(&self) -> Result<Vec<Track>, DbError> {
        queries::all_tracks(&self.db).await
    }

    pub async fn all_users(&self) -> Result<Vec<User>, DbError> {
        queries::all_users(&self.db).await
    }

    pub async fn create_user(
        &self,
        country: Option<String>,
        age: Option<f32>,
        gender: Option<String>,
    ) -> Result<User, DbError> {
        queries::create_user(&self.db, country, age, gender).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, track, user};
    use crate::domain::Track;

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
    
    fn named(index: u32, title: &str, artist: &str) -> Track {
        Track {
            track_index: index,
            artist_index: index,
            track_name: Some(title.to_string()),
            artist_name: Some(artist.to_string()),
        }
    }

    #[tokio::test]
    async fn users_come_with_play_counts_most_active_first() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store
            .add_users(vec![user(1), user(2), user(3)])
            .await
            .unwrap();
        store.add_tracks(vec![track(10), track(11)]).await.unwrap();
        store
            .add_events(vec![event_of(2, 10, 0), event_of(2, 11, 1), event_of(1, 10, 0)])
            .await
            .unwrap();

        let page = store.users_page(2, 0).await.unwrap();
        assert_eq!(page.total, 3);
        let seen: Vec<(u32, usize)> = page
            .items
            .iter()
            .map(|item| (item.user.user_index, item.plays))
            .collect();
        assert_eq!(seen, vec![(2, 2), (1, 1)]);

        let rest = store.users_page(2, 2).await.unwrap();
        assert_eq!(rest.items.len(), 1);
        assert_eq!(rest.items[0].user.user_index, 3);
        assert_eq!(rest.items[0].plays, 0);
    }

    #[tokio::test]
    async fn tracks_are_found_by_title_or_artist() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store
            .add_tracks(vec![
                named(1, "Crocodile", "Underworld"),
                named(2, "Born Slippy", "Underworld"),
                named(3, "Kids", "MGMT"),
            ])
            .await
            .unwrap();

        assert_eq!(store.search_tracks("underworld", 10).await.unwrap().len(), 2);
        assert_eq!(store.search_tracks("kid", 10).await.unwrap()[0].track_index, 3);
        assert_eq!(store.search_tracks("underworld", 1).await.unwrap().len(), 1);
        assert!(store.search_tracks("zzz", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn new_users_get_the_next_index() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let first = store.create_user(Some("UK".to_string()), None, None).await.unwrap();
        let second = store.create_user(None, Some(30.0), None).await.unwrap();

        assert_eq!((first.user_index, second.user_index), (0, 1));
        assert_eq!(store.user(1).await.unwrap().unwrap().age, Some(30.0));
    }
}