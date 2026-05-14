use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::IndexingReadModel;

#[derive(Debug, Error)]
pub enum IndexingRepositoryError {
    #[error("indexing repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait IndexingRepository: Send + Sync {
    async fn load(
        &self,
        indexing_id: Uuid,
    ) -> Result<Option<IndexingReadModel>, IndexingRepositoryError>;

    async fn save(&self, read_model: IndexingReadModel) -> Result<(), IndexingRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<IndexingReadModel>, IndexingRepositoryError>;
}
