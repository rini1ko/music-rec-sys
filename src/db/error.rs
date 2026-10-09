use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbError {
    #[error("user {0} not found")]
    UserNotFound(u32),
    #[error("track {0} not found")]
    TrackNotFound(u32),
}