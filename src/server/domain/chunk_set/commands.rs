use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;
use crate::server::domain::shared::Timestamp;

use super::chunk::Chunk;

pub struct CreateChunkSet {
    pub chunk_set_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub chunks: Vec<Chunk>,
    pub occurred_at: Timestamp,
}

pub struct PinChunkSet {
    pub chunk_set_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct UnpinChunkSet {
    pub chunk_set_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct DeleteChunkSet {
    pub chunk_set_id: Uuid,
    pub occurred_at: Timestamp,
}

pub enum ChunkSetCommand {
    CreateChunkSet(CreateChunkSet),
    PinChunkSet(PinChunkSet),
    UnpinChunkSet(UnpinChunkSet),
    DeleteChunkSet(DeleteChunkSet),
}

impl ChunkSetCommand {
    pub fn chunk_set_id(&self) -> Uuid {
        match self {
            ChunkSetCommand::CreateChunkSet(c) => c.chunk_set_id,
            ChunkSetCommand::PinChunkSet(c) => c.chunk_set_id,
            ChunkSetCommand::UnpinChunkSet(c) => c.chunk_set_id,
            ChunkSetCommand::DeleteChunkSet(c) => c.chunk_set_id,
        }
    }
}
