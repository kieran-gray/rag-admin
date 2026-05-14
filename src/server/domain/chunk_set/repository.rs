use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::entity::{Chunk, ChunkSet};

#[derive(Debug, Error)]
pub enum ChunkSetRepositoryError {
    #[error("chunk set repository error: {0}")]
    Internal(String),
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
}
