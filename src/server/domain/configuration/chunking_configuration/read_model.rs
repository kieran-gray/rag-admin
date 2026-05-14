use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfigurationReadModel {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}
