use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SourceDocumentError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    ValidationError(String),
    InvalidCommand(String),
}

impl fmt::Display for SourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceDocumentError::NotFound => write!(f, "source document not found"),
            SourceDocumentError::AlreadyExists => write!(f, "source document already exists"),
            SourceDocumentError::AlreadyDeleted => write!(f, "source document has been deleted"),
            SourceDocumentError::ValidationError(msg)
            | SourceDocumentError::InvalidCommand(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl Error for SourceDocumentError {}
