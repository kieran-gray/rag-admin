use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ChunkSetError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    PinnedCannotDelete,
    ValidationError(String),
}

impl fmt::Display for ChunkSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkSetError::NotFound => write!(f, "chunk set not found"),
            ChunkSetError::AlreadyExists => write!(f, "chunk set already exists"),
            ChunkSetError::AlreadyDeleted => write!(f, "chunk set has been deleted"),
            ChunkSetError::PinnedCannotDelete => {
                write!(f, "chunk set is pinned and cannot be deleted")
            }
            ChunkSetError::ValidationError(msg) => write!(f, "{msg}"),
        }
    }
}

impl Error for ChunkSetError {}
