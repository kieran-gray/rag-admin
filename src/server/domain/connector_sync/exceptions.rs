use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ConnectorSyncError {
    NotFound,
    AlreadyStarted,
    AlreadyTerminal,
    InvalidCommand(String),
    Internal(String),
}

impl fmt::Display for ConnectorSyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorSyncError::NotFound => write!(f, "connector sync not found"),
            ConnectorSyncError::AlreadyStarted => write!(f, "connector sync already started"),
            ConnectorSyncError::AlreadyTerminal => write!(f, "connector sync already terminal"),
            ConnectorSyncError::InvalidCommand(m) | ConnectorSyncError::Internal(m) => {
                write!(f, "{m}")
            }
        }
    }
}

impl Error for ConnectorSyncError {}
