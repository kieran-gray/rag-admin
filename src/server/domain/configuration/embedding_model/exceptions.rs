#[derive(Debug, Clone)]
pub enum EmbeddingModelCatalogError {
    NotFound,
    ValidationError(String),
    InvalidCommand(String),
}

impl std::fmt::Display for EmbeddingModelCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "embedding model not found"),
            Self::ValidationError(msg) => write!(f, "{msg}"),
            Self::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EmbeddingModelCatalogError {}
