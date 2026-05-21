use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::chunking::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSetSummaryDto {
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

impl ChunkSetSummaryDto {
    pub fn in_use(&self) -> bool {
        self.pinned || self.indexing_refs > 0 || self.variant_result_refs > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetChunkSetPinnedRequestDto {
    pub chunk_set_id: Uuid,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteChunkSetRequestDto {
    pub chunk_set_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcChunkSetsRequestDto {
    pub older_than_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcChunkSetsResponseDto {
    pub deleted: u64,
}
