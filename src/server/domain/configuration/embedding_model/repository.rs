use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::entity::EmbeddingModel;

#[derive(Debug, Error)]
pub enum EmbeddingModelRepositoryError {
    #[error("embedding model repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait EmbeddingModelRepository: Send + Sync {
    async fn load_all(&self) -> Result<Vec<EmbeddingModel>, EmbeddingModelRepositoryError>;

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<EmbeddingModel>, EmbeddingModelRepositoryError>;

    async fn save(&self, model: EmbeddingModel) -> Result<(), EmbeddingModelRepositoryError>;

    async fn delete(&self, model_id: Uuid) -> Result<(), EmbeddingModelRepositoryError>;
}
