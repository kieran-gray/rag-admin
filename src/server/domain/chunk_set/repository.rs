use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::chunk::Chunk;
use super::events::ChunkSetCreated;
use super::read_model::ChunkSetReadModel;

#[derive(Debug, Error)]
pub enum ChunkSetRepositoryError {
    #[error("chunk set repository error: {0}")]
    Internal(String),

    #[error("chunk set {0} is in use and cannot be deleted")]
    InUse(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkSetStatusFilter {
    Pinned,
    Indexed,
    UsedByEval,
    Unused,
}

#[derive(Debug, Clone)]
pub struct ChunkSetListCursor {
    pub created_at: OffsetDateTime,
    pub chunk_set_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct ChunkSetListQuery {
    pub cursor: Option<ChunkSetListCursor>,
    pub limit: u32,
    pub statuses: Vec<ChunkSetStatusFilter>,
}

#[derive(Debug, Clone)]
pub struct ChunkSetListPage {
    pub items: Vec<ChunkSetReadModel>,
    pub next_cursor: Option<ChunkSetListCursor>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<(ChunkSetStatusFilter, u64)>,
}

#[async_trait]
pub trait ChunkSetRepository: Send + Sync {
    async fn load_summary(
        &self,
        chunk_set_id: Uuid,
    ) -> Result<Option<ChunkSetReadModel>, ChunkSetRepositoryError>;

    async fn load_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<Chunk>, ChunkSetRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError>;

    async fn list_page(
        &self,
        query: &ChunkSetListQuery,
    ) -> Result<ChunkSetListPage, ChunkSetRepositoryError>;

    async fn project_created(&self, event: &ChunkSetCreated)
        -> Result<(), ChunkSetRepositoryError>;

    async fn project_pinned(
        &self,
        chunk_set_id: Uuid,
        pinned: bool,
    ) -> Result<(), ChunkSetRepositoryError>;

    async fn project_deleted(&self, chunk_set_id: Uuid) -> Result<(), ChunkSetRepositoryError>;

    async fn bump_indexing_refs(
        &self,
        chunk_set_id: Uuid,
        delta: i32,
    ) -> Result<(), ChunkSetRepositoryError>;

    async fn bump_variant_result_refs(
        &self,
        chunk_set_id: Uuid,
        delta: i32,
    ) -> Result<(), ChunkSetRepositoryError>;

    async fn reconcile_referrer_counts(&self) -> Result<(), ChunkSetRepositoryError>;

    async fn list_unused_older_than(
        &self,
        older_than_seconds: u64,
    ) -> Result<Vec<Uuid>, ChunkSetRepositoryError>;
}

impl From<ChunkSetRepositoryError> for ProjectionError {
    fn from(value: ChunkSetRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
