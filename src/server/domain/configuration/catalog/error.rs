use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum CatalogError {
    #[error("catalog entry not found")]
    NotFound,
    #[error("{0}")]
    ValidationError(String),
    #[error("invalid command: {0}")]
    InvalidCommand(String),
}
