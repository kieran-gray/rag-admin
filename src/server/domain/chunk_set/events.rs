use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;
use crate::server::domain::shared::Timestamp;

use super::chunk::Chunk;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkSetCreated {
    pub chunk_set_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub chunks: Vec<Chunk>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkSetPinned {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkSetUnpinned {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkSetDeleted {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum ChunkSetEvent {
    ChunkSetCreated(ChunkSetCreated),
    ChunkSetPinned(ChunkSetPinned),
    ChunkSetUnpinned(ChunkSetUnpinned),
    ChunkSetDeleted(ChunkSetDeleted),
}
