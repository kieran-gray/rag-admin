use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ConnectorError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    ValidationError(String),
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorError::NotFound => write!(f, "connector not found"),
            ConnectorError::AlreadyExists => write!(f, "connector already exists"),
            ConnectorError::AlreadyDeleted => write!(f, "connector has been deleted"),
            ConnectorError::ValidationError(msg) => write!(f, "{msg}"),
        }
    }
}

impl Error for ConnectorError {}
