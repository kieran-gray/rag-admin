use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::value_objects::ChunkingConfig;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkingConfigurationCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkingConfigurationAdded {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkingConfigurationUpdated {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkingConfigurationRemoved {
    pub chunking_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ChunkingConfigurationCatalogEvent {
    ChunkingConfigurationCatalogCreated(ChunkingConfigurationCatalogCreated),
    ChunkingConfigurationAdded(ChunkingConfigurationAdded),
    ChunkingConfigurationUpdated(ChunkingConfigurationUpdated),
    ChunkingConfigurationRemoved(ChunkingConfigurationRemoved),
}
