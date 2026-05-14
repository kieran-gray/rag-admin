#[derive(Debug, Clone)]
pub enum SourceDocumentError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    ValidationError(String),
    InvalidCommand(String),
}

impl std::fmt::Display for SourceDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceDocumentError::NotFound => write!(f, "source document not found"),
            SourceDocumentError::AlreadyExists => write!(f, "source document already exists"),
            SourceDocumentError::AlreadyDeleted => write!(f, "source document has been deleted"),
            SourceDocumentError::ValidationError(msg) => write!(f, "{msg}"),
            SourceDocumentError::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for SourceDocumentError {}
