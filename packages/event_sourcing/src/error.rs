use std::error::Error as StdError;
use std::fmt;

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum EsError {
    #[error(
        "{aggregate} stream {stream_id} version conflict: expected v{expected}, actual v{actual}"
    )]
    ConcurrencyConflict {
        aggregate: &'static str,
        stream_id: Uuid,
        expected: i64,
        actual: i64,
    },

    #[error("storage: {0}")]
    Storage(String),

    #[error("serialization: {0}")]
    Serialization(String),
}

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("storage: {0}")]
    Storage(String),

    #[error("invalid state: {0}")]
    InvalidState(String),
}

impl From<ProjectionError> for EsError {
    fn from(value: ProjectionError) -> Self {
        match value {
            ProjectionError::Storage(s) | ProjectionError::InvalidState(s) => EsError::Storage(s),
        }
    }
}

#[derive(Debug)]
pub enum CommandError<E> {
    Aggregate(E),
    EventStore(EsError),
}

impl<E: fmt::Display> fmt::Display for CommandError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aggregate(e) => write!(f, "{e}"),
            Self::EventStore(e) => write!(f, "{e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for CommandError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Aggregate(e) => Some(e),
            Self::EventStore(e) => Some(e),
        }
    }
}

impl<E> From<EsError> for CommandError<E> {
    fn from(value: EsError) -> Self {
        Self::EventStore(value)
    }
}
