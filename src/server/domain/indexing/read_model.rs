use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;

use super::status::IndexingStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingReadModel {
    pub indexing_id: Uuid,
    pub document_id: Uuid,
    pub index_profile_id: Uuid,
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
