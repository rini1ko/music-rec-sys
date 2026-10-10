use sea_orm::{DbErr, SqlErr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbError {
    #[error("user {0} not found")]
    UserNotFound(u32),
    #[error("track {0} not found")]
    TrackNotFound(u32),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("lost connection to the database: {0}")]
    Connection(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<DbErr> for DbError {
    fn from(error: DbErr) -> Self {
        match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(message))
            | Some(SqlErr::ForeignKeyConstraintViolation(message)) => DbError::Conflict(message),
            _ => match error {
                DbErr::ConnectionAcquire(_) | DbErr::Conn(_) => {
                    DbError::Connection(error.to_string())
                }
                other => DbError::Storage(other.to_string()),
            },
        }
    }
}