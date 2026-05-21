use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::entity::{Chunk, ChunkSet};
use super::read_model::ChunkSetReadModel;

#[derive(Debug, Error)]
pub enum ChunkSetRepositoryError {
    #[error("chunk set repository error: {0}")]
    Internal(String),

    #[error("chunk set {0} is in use and cannot be deleted")]
    InUse(Uuid),
}

#[async_trait]
pub trait ChunkSetRepository: Send + Sync {
    async fn save(
        &self,
        chunk_set: ChunkSet,
        chunks: Vec<Chunk>,
    ) -> Result<(), ChunkSetRepositoryError>;

    async fn load(&self, chunk_set_id: Uuid) -> Result<Option<ChunkSet>, ChunkSetRepositoryError>;

    async fn load_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<Chunk>, ChunkSetRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSet>, ChunkSetRepositoryError>;

    async fn delete(&self, chunk_set_id: Uuid) -> Result<(), ChunkSetRepositoryError>;

    async fn set_pinned(
        &self,
        chunk_set_id: Uuid,
        pinned: bool,
    ) -> Result<(), ChunkSetRepositoryError>;

    async fn delete_unused(
        &self,
        older_than_seconds: u64,
    ) -> Result<u64, ChunkSetRepositoryError>;

    async fn list_all_with_referrers(
        &self,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError>;

    async fn list_for_document_with_referrers(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError>;
}
