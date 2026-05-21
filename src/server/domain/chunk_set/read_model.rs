use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSetReadModel {
    pub chunk_set_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub created_at: String,
    pub pinned: bool,
    pub chunk_count: u32,
    pub indexing_refs: u32,
    pub variant_result_refs: u32,
}

impl ChunkSetReadModel {
    pub fn in_use(&self) -> bool {
        self.pinned || self.indexing_refs > 0 || self.variant_result_refs > 0
    }
}
