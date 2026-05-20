use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::ChunkingConfig;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddChunkingConfiguration {
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateChunkingConfiguration {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveChunkingConfiguration {
    pub chunking_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ChunkingConfigurationCatalogCommand {
    AddChunkingConfiguration(AddChunkingConfiguration),
    UpdateChunkingConfiguration(UpdateChunkingConfiguration),
    RemoveChunkingConfiguration(RemoveChunkingConfiguration),
}
