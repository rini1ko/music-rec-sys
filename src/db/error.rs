use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbError {
    #[error("user {0} not found")]
    UserNotFound(u32),
    #[error("track {0} not found")]
    TrackNotFound(u32),
    /// Anything the database itself reported. Split into finer cases in the next step.
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<sea_orm::DbErr> for DbError {
    fn from(error: sea_orm::DbErr) -> Self {
        DbError::Storage(error.to_string())
    }
}