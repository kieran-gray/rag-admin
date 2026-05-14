use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::ChunkingConfig;

use super::{aggregate::Indexing, status::IndexingStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingReadModel {
    pub indexing_id: Uuid,
    pub document_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub chunk_set_id: Option<Uuid>,
    pub embedding_set_id: Option<Uuid>,
    pub status: IndexingStatus,
    pub attempts: u32,
    pub removed: bool,
    #[serde(default = "default_true")]
    pub auto_advance: bool,
}

fn default_true() -> bool {
    true
}

impl From<&Indexing> for IndexingReadModel {
    fn from(indexing: &Indexing) -> Self {
        Self {
            indexing_id: indexing.indexing_id,
            document_id: indexing.document_id,
            pipeline_configuration_id: indexing.pipeline_configuration_id,
            document_version: indexing.document_version,
            chunking_config: indexing.chunking_config,
            chunk_set_id: indexing.chunk_set_id,
            embedding_set_id: indexing.embedding_set_id,
            status: indexing.status.clone(),
            attempts: indexing.attempts,
            removed: indexing.removed,
            auto_advance: indexing.auto_advance,
        }
    }
}
