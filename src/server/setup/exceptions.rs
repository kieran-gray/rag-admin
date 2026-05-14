use thiserror::Error;

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("missing required environment variable: {0}")]
    MissingVariable(String),
    #[error("invalid environment variable: {0}")]
    InvalidVariable(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}
