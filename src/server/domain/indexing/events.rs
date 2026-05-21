use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;
use crate::server::domain::shared::Timestamp;

use super::status::IngestStage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestRequested {
    pub document_id: Uuid,
    pub index_profile_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub request_id: Uuid,
    #[serde(default = "default_auto_advance")]
    pub auto_advance: bool,
    pub occurred_at: Timestamp,
}

fn default_auto_advance() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkingCompleted {
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    #[serde(default = "default_auto_advance")]
    pub auto_advance: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingCompleted {
    pub embedding_set_id: Uuid,
    #[serde(default = "default_auto_advance")]
    pub auto_advance: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexingCompleted {
    pub vector_count: u32,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestionFailed {
    pub stage: IngestStage,
    pub reason: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestionRetried {
    pub request_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexingRemoved {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkingRequeued {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingRequeued {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexingRequeued {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum IndexingEvent {
    IngestRequested(IngestRequested),
    ChunkingCompleted(ChunkingCompleted),
    EmbeddingCompleted(EmbeddingCompleted),
    IndexingCompleted(IndexingCompleted),
    IngestionFailed(IngestionFailed),
    IngestionRetried(IngestionRetried),
    IndexingRemoved(IndexingRemoved),
    ChunkingRequeued(ChunkingRequeued),
    EmbeddingRequeued(EmbeddingRequeued),
    IndexingRequeued(IndexingRequeued),
}
