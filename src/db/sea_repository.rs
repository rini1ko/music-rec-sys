use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Duration, NaiveDateTime};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    Set, TransactionTrait,
};
use tokio::runtime::{Builder, Runtime};

use crate::db::entities::{listen_event, session, track, user};
use crate::db::error::DbError;
use crate::db::repository::{Counts, Repository};
use crate::db::schema::{connect, create_schema};
use crate::domain::{ListenEvent, Session, Track, User, UserSummary, UsersPage};

/// Rows per INSERT / per `IN (...)` query. SQLite allows about 32k bound
/// values in one statement and the widest table here has 4 columns.
const CHUNK: usize = 1000;

/// Repository backed by SQLite through sea-orm.
///
/// The `Repository` trait is synchronous, so every call runs on a private
/// tokio runtime with `block_on`. That panics if it is called from inside
/// another tokio runtime (we will deal with it when the axum API comes).
pub struct SeaOrmRepository {
    runtime: Runtime,
    db: DatabaseConnection,
}

impl SeaOrmRepository {
    /// `url` is e.g. `sqlite://data/music.db?mode=rwc`. Creates the tables if needed.
    pub fn open(url: &str) -> Result<Self, DbError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| DbError::Storage(error.to_string()))?;

        let db = runtime.block_on(async {
            let db = connect(url).await?;
            create_schema(&db).await?;
            Ok::<_, DbErr>(db)
        })?;

        Ok(Self { runtime, db })
    }
}

impl Repository for SeaOrmRepository {
    fn add_users(&mut self, users: Vec<User>) -> Result<(), DbError> {
        self.runtime.block_on(insert_users(&self.db, users))
    }

    fn add_tracks(&mut self, tracks: Vec<Track>) -> Result<(), DbError> {
        self.runtime.block_on(insert_tracks(&self.db, tracks))
    }

    fn add_events(&mut self, events: Vec<ListenEvent>) -> Result<(), DbError> {
        self.runtime.block_on(insert_events(&self.db, events))
    }

    fn add_sessions(&mut self, sessions: Vec<Session>) -> Result<(), DbError> {
        self.runtime.block_on(insert_sessions(&self.db, sessions))
    }

    fn replace_sessions(&mut self, sessions: Vec<Session>) -> Result<(), DbError> {
        self.runtime.block_on(replace_sessions(&self.db, sessions))
    }

    fn all_events(&self) -> Result<Vec<ListenEvent>, DbError> {
        self.runtime.block_on(all_events(&self.db))
    }

    fn all_sessions(&self) -> Result<Vec<Session>, DbError> {
        self.runtime.block_on(all_sessions(&self.db))
    }

    fn all_tracks(&self) -> Result<Vec<Track>, DbError> {
        self.runtime.block_on(all_tracks(&self.db))
    }

    fn all_users(&self) -> Result<Vec<User>, DbError> {
        self.runtime.block_on(all_users(&self.db))
    }

    fn user_history(&self, user_index: u32) -> Result<Vec<u32>, DbError> {
        self.runtime.block_on(history(&self.db, user_index))
    }

    fn trending(&self, window: Duration, limit: usize) -> Result<Vec<(u32, usize)>, DbError> {
        self.runtime.block_on(trending(&self.db, window, limit))
    }

    fn get_user(&self, user_index: u32) -> Result<Option<User>, DbError> {
        self.runtime.block_on(find_user(&self.db, user_index))
    }

    fn get_track(&self, track_index: u32) -> Result<Option<Track>, DbError> {
        self.runtime.block_on(find_track(&self.db, track_index))
    }

    fn counts(&self) -> Result<Counts, DbError> {
        self.runtime.block_on(count_rows(&self.db))
    }
}

// ---------- writes ----------

pub(crate) async fn insert_users(db: &DatabaseConnection, users: Vec<User>) -> Result<(), DbError> {
    let txn = db.begin().await?;

    // the first version of a user wins: inside the batch and against stored rows
    let mut fresh: BTreeMap<u32, User> = BTreeMap::new();
    for user in users {
        fresh.entry(user.user_index).or_insert(user);
    }
    let ids: Vec<u32> = fresh.keys().copied().collect();
    for stored in existing_users(&txn, &ids).await? {
        fresh.remove(&stored);
    }

    let rows: Vec<user::ActiveModel> = fresh.into_values().map(user_row).collect();
    for chunk in rows.chunks(CHUNK) {
        user::Entity::insert_many(chunk.to_vec()).exec(&txn).await?;
    }

    txn.commit().await?;
    Ok(())
}

pub(crate) async fn insert_tracks(
    db: &DatabaseConnection,
    tracks: Vec<Track>,
) -> Result<(), DbError> {
    let txn = db.begin().await?;

    let mut fresh: BTreeMap<u32, Track> = BTreeMap::new();
    for track in tracks {
        fresh.entry(track.track_index).or_insert(track);
    }
    let ids: Vec<u32> = fresh.keys().copied().collect();
    for stored in existing_tracks(&txn, &ids).await? {
        fresh.remove(&stored);
    }

    let rows: Vec<track::ActiveModel> = fresh.into_values().map(track_row).collect();
    for chunk in rows.chunks(CHUNK) {
        track::Entity::insert_many(chunk.to_vec())
            .exec(&txn)
            .await?;
    }

    txn.commit().await?;
    Ok(())
}

pub(crate) async fn insert_events(
    db: &DatabaseConnection,
    events: Vec<ListenEvent>,
) -> Result<(), DbError> {
    // if anything below fails, `txn` is dropped and rolled back
    let txn = db.begin().await?;

    ensure_users(&txn, events.iter().map(|event| event.user_index).collect()).await?;
    ensure_tracks(&txn, events.iter().map(|event| event.track_index).collect()).await?;

    for chunk in events.chunks(CHUNK) {
        let rows: Vec<listen_event::ActiveModel> = chunk.iter().map(event_row).collect();
        listen_event::Entity::insert_many(rows).exec(&txn).await?;
    }

    txn.commit().await?;
    Ok(())
}

/// Deletes the stored sessions and inserts `sessions` in one transaction.
pub(crate) async fn replace_sessions(
    db: &DatabaseConnection,
    sessions: Vec<Session>,
) -> Result<(), DbError> {
    let txn = db.begin().await?;
    session::Entity::delete_many().exec(&txn).await?;
    store_sessions(&txn, sessions).await?;
    txn.commit().await?;
    Ok(())
}

pub(crate) async fn insert_sessions(
    db: &DatabaseConnection,
    sessions: Vec<Session>,
) -> Result<(), DbError> {
    let txn = db.begin().await?;
    store_sessions(&txn, sessions).await?;
    txn.commit().await?;
    Ok(())
}

/// Checks the users and tracks of `sessions` and inserts them in chunks.
async fn store_sessions<C: ConnectionTrait>(
    conn: &C,
    sessions: Vec<Session>,
) -> Result<(), DbError> {
    ensure_users(conn, sessions.iter().map(|item| item.user_index).collect()).await?;
    ensure_tracks(
        conn,
        sessions
            .iter()
            .flat_map(|item| item.tracks.iter().copied())
            .collect(),
    )
    .await?;

    for chunk in sessions.chunks(CHUNK) {
        let rows = chunk
            .iter()
            .map(session_row)
            .collect::<Result<Vec<_>, _>>()?;
        session::Entity::insert_many(rows).exec(conn).await?;
    }
    Ok(())
}

pub(crate) async fn all_tracks(db: &DatabaseConnection) -> Result<Vec<Track>, DbError> {
    let rows = track::Entity::find().all(db).await?;
    Ok(rows.into_iter().map(track_from).collect())
}

pub(crate) async fn all_users(db: &DatabaseConnection) -> Result<Vec<User>, DbError> {
    let rows = user::Entity::find().all(db).await?;
    Ok(rows.into_iter().map(user_from).collect())
}

/// The new user gets the next free index.
pub(crate) async fn create_user(
    db: &DatabaseConnection,
    country: Option<String>,
    age: Option<f32>,
    gender: Option<String>,
) -> Result<User, DbError> {
    let txn = db.begin().await?;

    let latest = user::Entity::find()
        .order_by_desc(user::Column::UserIndex)
        .one(&txn)
        .await?;
    let created = User {
        user_index: latest.map_or(0, |row| row.user_index + 1),
        gender,
        age,
        country,
    };
    user::Entity::insert(user_row(created.clone()))
        .exec(&txn)
        .await?;

    txn.commit().await?;
    Ok(created)
}

// ---------- reads and checks ----------

#[derive(FromQueryResult)]
struct TrackPlays {
    track_index: u32,
    plays: i64,
}

pub(crate) async fn trending(
    db: &DatabaseConnection,
    window: Duration,
    limit: usize,
) -> Result<Vec<(u32, usize)>, DbError> {
    let Some(latest) = latest_event_time(db).await? else {
        return Ok(Vec::new());
    };

    let rows = listen_event::Entity::find()
        .select_only()
        .column(listen_event::Column::TrackIndex)
        .column_as(listen_event::Column::Id.count(), "plays")
        .filter(listen_event::Column::Timestamp.gte(latest - window))
        .group_by(listen_event::Column::TrackIndex)
        .order_by_desc(listen_event::Column::Id.count())
        .order_by_asc(listen_event::Column::TrackIndex)
        .limit(limit as u64)
        .into_model::<TrackPlays>()
        .all(db)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.track_index, row.plays as usize))
        .collect())
}

pub(crate) async fn history(db: &DatabaseConnection, user_index: u32) -> Result<Vec<u32>, DbError> {
    if user::Entity::find_by_id(user_index)
        .one(db)
        .await?
        .is_none()
    {
        return Err(DbError::UserNotFound(user_index));
    }

    let rows = listen_event::Entity::find()
        .filter(listen_event::Column::UserIndex.eq(user_index))
        .order_by_asc(listen_event::Column::Timestamp)
        .order_by_asc(listen_event::Column::Id)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.track_index).collect())
}

pub(crate) async fn count_rows(db: &DatabaseConnection) -> Result<Counts, DbError> {
    Ok(Counts {
        users: user::Entity::find().count(db).await? as usize,
        tracks: track::Entity::find().count(db).await? as usize,
        events: listen_event::Entity::find().count(db).await? as usize,
        sessions: session::Entity::find().count(db).await? as usize,
    })
}

/// Which of the given user ids are stored.
async fn existing_users<C: ConnectionTrait>(
    conn: &C,
    wanted: &[u32],
) -> Result<BTreeSet<u32>, DbErr> {
    let mut found = BTreeSet::new();
    for chunk in wanted.chunks(CHUNK) {
        let rows = user::Entity::find()
            .filter(user::Column::UserIndex.is_in(chunk.iter().copied()))
            .all(conn)
            .await?;
        found.extend(rows.into_iter().map(|row| row.user_index));
    }
    Ok(found)
}

/// Which of the given track ids are stored.
async fn existing_tracks<C: ConnectionTrait>(
    conn: &C,
    wanted: &[u32],
) -> Result<BTreeSet<u32>, DbErr> {
    let mut found = BTreeSet::new();
    for chunk in wanted.chunks(CHUNK) {
        let rows = track::Entity::find()
            .filter(track::Column::TrackIndex.is_in(chunk.iter().copied()))
            .all(conn)
            .await?;
        found.extend(rows.into_iter().map(|row| row.track_index));
    }
    Ok(found)
}

async fn ensure_users<C: ConnectionTrait>(conn: &C, ids: BTreeSet<u32>) -> Result<(), DbError> {
    let wanted: Vec<u32> = ids.into_iter().collect();
    let found = existing_users(conn, &wanted).await?;
    match wanted.into_iter().find(|id| !found.contains(id)) {
        Some(missing) => Err(DbError::UserNotFound(missing)),
        None => Ok(()),
    }
}

async fn ensure_tracks<C: ConnectionTrait>(conn: &C, ids: BTreeSet<u32>) -> Result<(), DbError> {
    let wanted: Vec<u32> = ids.into_iter().collect();
    let found = existing_tracks(conn, &wanted).await?;
    match wanted.into_iter().find(|id| !found.contains(id)) {
        Some(missing) => Err(DbError::TrackNotFound(missing)),
        None => Ok(()),
    }
}

pub(crate) async fn all_events(db: &DatabaseConnection) -> Result<Vec<ListenEvent>, DbError> {
    // same order as MemoryRepository: by user, then as inserted
    let rows = listen_event::Entity::find()
        .order_by_asc(listen_event::Column::UserIndex)
        .order_by_asc(listen_event::Column::Id)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(event_from).collect())
}

pub(crate) async fn all_sessions(db: &DatabaseConnection) -> Result<Vec<Session>, DbError> {
    let rows = session::Entity::find()
        .order_by_asc(session::Column::Id)
        .all(db)
        .await?;
    rows.into_iter().map(session_from).collect()
}

pub(crate) async fn find_user(
    db: &DatabaseConnection,
    user_index: u32,
) -> Result<Option<User>, DbError> {
    Ok(user::Entity::find_by_id(user_index)
        .one(db)
        .await?
        .map(user_from))
}

pub(crate) async fn find_track(
    db: &DatabaseConnection,
    track_index: u32,
) -> Result<Option<Track>, DbError> {
    Ok(track::Entity::find_by_id(track_index)
        .one(db)
        .await?
        .map(track_from))
}

/// The stored tracks among `wanted`, by index. Unknown indexes are left out.
pub(crate) async fn find_tracks(
    db: &DatabaseConnection,
    wanted: &[u32],
) -> Result<HashMap<u32, Track>, DbError> {
    let mut found = HashMap::with_capacity(wanted.len());
    for chunk in wanted.chunks(CHUNK) {
        let rows = track::Entity::find()
            .filter(track::Column::TrackIndex.is_in(chunk.iter().copied()))
            .all(db)
            .await?;
        found.extend(
            rows.into_iter()
                .map(|row| (row.track_index, track_from(row))),
        );
    }
    Ok(found)
}

/// The user's last `limit` plays, newest first (the user-time index serves it).
pub(crate) async fn recent_events(
    db: &DatabaseConnection,
    user_index: u32,
    limit: usize,
) -> Result<Vec<ListenEvent>, DbError> {
    let rows = listen_event::Entity::find()
        .filter(listen_event::Column::UserIndex.eq(user_index))
        .order_by_desc(listen_event::Column::Timestamp)
        .order_by_desc(listen_event::Column::Id)
        .limit(limit as u64)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(event_from).collect())
}

pub(crate) async fn latest_event_time(
    db: &DatabaseConnection,
) -> Result<Option<NaiveDateTime>, DbError> {
    // newest event through the timestamp index, no full scan
    Ok(listen_event::Entity::find()
        .order_by_desc(listen_event::Column::Timestamp)
        .one(db)
        .await?
        .map(|row| row.timestamp))
}

#[derive(FromQueryResult)]
struct UserRow {
    user_index: u32,
    gender: Option<String>,
    age: Option<f32>,
    country: Option<String>,
    plays: i64,
}

/// Most active users first. The join and the count read only the events index.
pub(crate) async fn users_page(
    db: &DatabaseConnection,
    limit: usize,
    offset: usize,
) -> Result<UsersPage, DbError> {
    let total = user::Entity::find().count(db).await? as usize;

    let rows = user::Entity::find()
        .select_only()
        .column(user::Column::UserIndex)
        .column(user::Column::Gender)
        .column(user::Column::Age)
        .column(user::Column::Country)
        .column_as(listen_event::Column::Id.count(), "plays")
        .join_rev(JoinType::LeftJoin, listen_event::Relation::User.def())
        .group_by(user::Column::UserIndex)
        .order_by_desc(listen_event::Column::Id.count())
        .order_by_asc(user::Column::UserIndex)
        .limit(limit as u64)
        .offset(offset as u64)
        .into_model::<UserRow>()
        .all(db)
        .await?;

    let items = rows
        .into_iter()
        .map(|row| UserSummary {
            user: User {
                user_index: row.user_index,
                gender: row.gender,
                age: row.age,
                country: row.country,
            },
            plays: row.plays as usize,
        })
        .collect();
    Ok(UsersPage { total, items })
}

/// Tracks whose title or artist contains the text (SQLite LIKE ignores case for ASCII).
pub(crate) async fn search_tracks(
    db: &DatabaseConnection,
    text: &str,
    limit: usize,
) -> Result<Vec<Track>, DbError> {
    let rows = track::Entity::find()
        .filter(
            Condition::any()
                .add(track::Column::TrackName.contains(text))
                .add(track::Column::ArtistName.contains(text)),
        )
        .order_by_asc(track::Column::ArtistName)
        .order_by_asc(track::Column::TrackName)
        .limit(limit as u64)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(track_from).collect())
}

// ---------- domain <-> table rows ----------

fn user_row(item: User) -> user::ActiveModel {
    user::ActiveModel {
        user_index: Set(item.user_index),
        gender: Set(item.gender),
        age: Set(item.age),
        country: Set(item.country),
    }
}

fn user_from(row: user::Model) -> User {
    User {
        user_index: row.user_index,
        gender: row.gender,
        age: row.age,
        country: row.country,
    }
}

fn track_row(item: Track) -> track::ActiveModel {
    track::ActiveModel {
        track_index: Set(item.track_index),
        artist_index: Set(item.artist_index),
        track_name: Set(item.track_name),
        artist_name: Set(item.artist_name),
    }
}

fn track_from(row: track::Model) -> Track {
    Track {
        track_index: row.track_index,
        artist_index: row.artist_index,
        track_name: row.track_name,
        artist_name: row.artist_name,
    }
}

fn event_row(item: &ListenEvent) -> listen_event::ActiveModel {
    listen_event::ActiveModel {
        user_index: Set(item.user_index),
        track_index: Set(item.track_index),
        timestamp: Set(item.timestamp),
        ..Default::default()
    }
}

fn event_from(row: listen_event::Model) -> ListenEvent {
    ListenEvent {
        user_index: row.user_index,
        track_index: row.track_index,
        timestamp: row.timestamp,
    }
}

fn session_row(item: &Session) -> Result<session::ActiveModel, DbError> {
    let tracks = serde_json::to_value(&item.tracks)
        .map_err(|error| DbError::Storage(format!("cannot store session tracks: {error}")))?;
    Ok(session::ActiveModel {
        user_index: Set(item.user_index),
        started_at: Set(item.start),
        ended_at: Set(item.end),
        tracks: Set(tracks),
        ..Default::default()
    })
}

fn session_from(row: session::Model) -> Result<Session, DbError> {
    let tracks: Vec<u32> = serde_json::from_value(row.tracks)
        .map_err(|error| DbError::Storage(format!("corrupt session tracks: {error}")))?;
    Ok(Session {
        user_index: row.user_index,
        tracks,
        start: row.started_at,
        end: row.ended_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, session_of, track, user};

    fn repo_with(users: &[u32], tracks: &[u32]) -> SeaOrmRepository {
        let mut repo = SeaOrmRepository::open("sqlite::memory:").unwrap();
        repo.add_users(users.iter().map(|&u| user(u)).collect())
            .unwrap();
        repo.add_tracks(tracks.iter().map(|&t| track(t)).collect())
            .unwrap();
        repo
    }

    #[test]
    fn events_need_a_known_user() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(2, 10, 0)]);
        assert_eq!(result, Err(DbError::UserNotFound(2)));
    }

    #[test]
    fn events_need_a_known_track() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(1, 99, 0)]);
        assert_eq!(result, Err(DbError::TrackNotFound(99)));
    }

    #[test]
    fn failed_batch_stores_nothing() {
        let mut repo = repo_with(&[1], &[10]);
        let result = repo.add_events(vec![event_of(1, 10, 0), event_of(1, 99, 1)]);

        assert!(result.is_err());
        assert_eq!(repo.counts().unwrap().events, 0);
    }

    #[test]
    fn sessions_need_known_tracks() {
        let mut repo = repo_with(&[0], &[10]);
        let result = repo.add_sessions(vec![session_of(0, &[10, 99])]);

        assert_eq!(result, Err(DbError::TrackNotFound(99)));
        assert_eq!(repo.counts().unwrap().sessions, 0);
    }

    #[test]
    fn first_version_of_a_user_wins() {
        let mut repo = SeaOrmRepository::open("sqlite::memory:").unwrap();
        let mut first = user(1);
        first.country = Some("UK".to_string());
        let mut second = user(1);
        second.country = Some("DE".to_string());

        repo.add_users(vec![first]).unwrap();
        repo.add_users(vec![second]).unwrap();

        let stored = repo.get_user(1).unwrap().unwrap();
        assert_eq!(stored.country, Some("UK".to_string()));
    }

    #[test]
    fn history_is_ordered_by_time() {
        let mut repo = repo_with(&[1], &[10, 11, 12]);
        repo.add_events(vec![
            event_of(1, 10, 5),
            event_of(1, 11, 0),
            event_of(1, 12, 3),
        ])
        .unwrap();

        assert_eq!(repo.user_history(1), Ok(vec![11, 12, 10]));
    }

    #[test]
    fn history_of_unknown_user_is_an_error() {
        let repo = repo_with(&[], &[]);
        assert_eq!(repo.user_history(7), Err(DbError::UserNotFound(7)));
    }

    #[test]
    fn user_without_events_has_empty_history() {
        let repo = repo_with(&[1], &[]);
        assert_eq!(repo.user_history(1), Ok(Vec::new()));
    }

    #[test]
    fn session_tracks_keep_their_order() {
        let mut repo = repo_with(&[1], &[10, 11, 12]);
        repo.add_sessions(vec![session_of(1, &[12, 10, 11])])
            .unwrap();

        assert_eq!(
            repo.all_sessions().unwrap(),
            vec![session_of(1, &[12, 10, 11])]
        );
    }

    #[test]
    fn big_batches_are_split_into_chunks() {
        let ids: Vec<u32> = (0..2500).collect();
        let mut repo = repo_with(&[1], &ids);
        let events = ids.iter().map(|&t| event_of(1, t, i64::from(t))).collect();
        repo.add_events(events).unwrap();

        let counts = repo.counts().unwrap();
        assert_eq!((counts.tracks, counts.events), (2500, 2500));
    }

    #[test]
    fn counts_reflect_stored_data() {
        let mut repo = repo_with(&[1, 2], &[10, 11]);
        repo.add_events(vec![
            event_of(1, 10, 0),
            event_of(2, 11, 0),
            event_of(2, 10, 1),
        ])
        .unwrap();
        repo.add_sessions(vec![session_of(1, &[10, 11])]).unwrap();

        assert_eq!(
            repo.counts().unwrap(),
            Counts {
                users: 2,
                tracks: 2,
                events: 3,
                sessions: 1,
            }
        );
    }
    #[test]
    fn duplicate_key_is_a_conflict() {
        let repo = repo_with(&[1], &[]);
        let error = repo
            .runtime
            .block_on(user::Entity::insert(user_row(user(1))).exec(&repo.db))
            .unwrap_err();

        assert!(matches!(DbError::from(error), DbError::Conflict(_)));
    }

    #[test]
    fn trending_counts_back_from_the_latest_event() {
        let mut repo = repo_with(&[1], &[1, 2, 3]);
        repo.add_events(vec![
            event_of(1, 1, 0),
            event_of(1, 3, 10),
            event_of(1, 2, 2 * 24 * 60 - 10),
            event_of(1, 2, 2 * 24 * 60),
        ])
        .unwrap();

        assert_eq!(repo.trending(Duration::days(1), 5), Ok(vec![(2, 2)]));
    }
}
