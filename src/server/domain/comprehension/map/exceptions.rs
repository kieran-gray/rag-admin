use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum DocumentMapError {
    NotFound,
    AlreadyExists,
    AlreadyReady,
    InvalidTransition(String),
    ValidationError(String),
}

impl fmt::Display for DocumentMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "document map not found"),
            Self::AlreadyExists => write!(f, "document map already exists"),
            Self::AlreadyReady => write!(f, "document map is already ready"),
            Self::InvalidTransition(msg) => write!(f, "invalid map transition: {msg}"),
            Self::ValidationError(msg) => write!(f, "{msg}"),
        }
    }
}

impl Error for DocumentMapError {}
