use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ConfigurationDefaultsError {
    #[error("{0}")]
    ValidationError(String),
}
