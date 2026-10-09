use crate::db::error::DbError;
use crate::domain::{ListenEvent, Session, Track, User};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub users: usize,
    pub tracks: usize,
    pub events: usize,
    pub sessions: usize,
}

/// The only way the rest of the program touches stored data.
///
/// Writes are batch-oriented and must be atomic: if any item of a batch is
/// invalid, nothing from that batch is stored.
pub trait Repository {
    /// Stores new users. A user that already exists keeps its first version.
    fn add_users(&mut self, users: Vec<User>) -> Result<(), DbError>;

    /// Stores new tracks. A track that already exists keeps its first version.
    fn add_tracks(&mut self, tracks: Vec<Track>) -> Result<(), DbError>;

    /// Every event must reference a stored user and a stored track.
    fn add_events(&mut self, events: Vec<ListenEvent>) -> Result<(), DbError>;

    /// Every session must reference a stored user and stored tracks.
    fn add_sessions(&mut self, sessions: Vec<Session>) -> Result<(), DbError>;

    fn all_events(&self) -> Result<Vec<ListenEvent>, DbError>;

    fn all_sessions(&self) -> Result<Vec<Session>, DbError>;

    /// Tracks the user listened to, oldest first. A known user without
    /// events gets an empty list, an unknown user gets `UserNotFound`.
    fn user_history(&self, user_index: u32) -> Result<Vec<u32>, DbError>;

    fn get_user(&self, user_index: u32) -> Result<Option<User>, DbError>;

    fn get_track(&self, track_index: u32) -> Result<Option<Track>, DbError>;

    fn counts(&self) -> Result<Counts, DbError>;
}