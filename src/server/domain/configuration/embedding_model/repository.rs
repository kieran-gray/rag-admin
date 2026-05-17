use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;

use super::entity::EmbeddingModel;
use crate::event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum EmbeddingModelRepositoryError {
    #[error("embedding model repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait EmbeddingModelRepository: CatalogRepository<EmbeddingModel> {
    async fn load_all(&self) -> Result<Vec<EmbeddingModel>, EmbeddingModelRepositoryError>;

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<EmbeddingModel>, EmbeddingModelRepositoryError>;
}

impl From<EmbeddingModelRepositoryError> for ProjectionError {
    fn from(value: EmbeddingModelRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
