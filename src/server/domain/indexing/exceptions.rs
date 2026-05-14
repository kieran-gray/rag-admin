#[derive(Debug, Clone)]
pub enum IndexingError {
    NotFound,
    Removed,
    NotFailed,
    ValidationError(String),
    InvalidCommand(String),
}

impl std::fmt::Display for IndexingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexingError::NotFound => write!(f, "indexing not found"),
            IndexingError::Removed => write!(f, "indexing has been removed"),
            IndexingError::NotFailed => write!(f, "indexing is not in a failed state"),
            IndexingError::ValidationError(msg) => write!(f, "{msg}"),
            IndexingError::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for IndexingError {}
