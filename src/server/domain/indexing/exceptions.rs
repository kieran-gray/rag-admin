use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum IndexingError {
    NotFound,
    Removed,
    NotFailed,
    ValidationError(String),
    InvalidCommand(String),
}

impl fmt::Display for IndexingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexingError::NotFound => write!(f, "indexing not found"),
            IndexingError::Removed => write!(f, "indexing has been removed"),
            IndexingError::NotFailed => write!(f, "indexing is not in a failed state"),
            IndexingError::ValidationError(msg) | IndexingError::InvalidCommand(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl Error for IndexingError {}
