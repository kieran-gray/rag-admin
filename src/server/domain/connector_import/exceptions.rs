use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ConnectorImportError {
    Internal(String),
}

impl fmt::Display for ConnectorImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorImportError::Internal(m) => write!(f, "{m}"),
        }
    }
}

impl Error for ConnectorImportError {}
