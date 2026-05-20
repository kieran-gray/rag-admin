use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PipelineConfigurationCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PipelineConfigurationAdded {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PipelineConfigurationUpdated {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PipelineConfigurationRemoved {
    pub pipeline_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum PipelineConfigurationCatalogEvent {
    PipelineConfigurationCatalogCreated(PipelineConfigurationCatalogCreated),
    PipelineConfigurationAdded(PipelineConfigurationAdded),
    PipelineConfigurationUpdated(PipelineConfigurationUpdated),
    PipelineConfigurationRemoved(PipelineConfigurationRemoved),
}
