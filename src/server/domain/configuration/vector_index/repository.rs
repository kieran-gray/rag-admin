use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::entity::VectorIndex;

#[derive(Debug, Error)]
pub enum VectorIndexRepositoryError {
    #[error("vector index repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait VectorIndexRepository: Send + Sync {
    async fn load_all(&self) -> Result<Vec<VectorIndex>, VectorIndexRepositoryError>;

    async fn find_by_id(
        &self,
        index_id: Uuid,
    ) -> Result<Option<VectorIndex>, VectorIndexRepositoryError>;

    async fn save(&self, index: VectorIndex) -> Result<(), VectorIndexRepositoryError>;

    async fn delete(&self, index_id: Uuid) -> Result<(), VectorIndexRepositoryError>;
}
