use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ConnectorImportError {
    NotFound,
    AlreadyStarted,
    AlreadyTerminal,
    InvalidCommand(String),
    Internal(String),
}

impl fmt::Display for ConnectorImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorImportError::NotFound => write!(f, "connector import not found"),
            ConnectorImportError::AlreadyStarted => write!(f, "connector import already started"),
            ConnectorImportError::AlreadyTerminal => write!(f, "connector import already terminal"),
            ConnectorImportError::InvalidCommand(m) | ConnectorImportError::Internal(m) => {
                write!(f, "{m}")
            }
        }
    }
}

impl Error for ConnectorImportError {}
