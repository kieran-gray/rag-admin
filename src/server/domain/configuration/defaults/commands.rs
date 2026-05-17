use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultChunkingConfiguration {
    pub chunking_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultPipelineConfiguration {
    pub pipeline_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultSweepTemplate {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConfigurationDefaultsCommand {
    SetDefaultChunkingConfiguration(SetDefaultChunkingConfiguration),
    SetDefaultPipelineConfiguration(SetDefaultPipelineConfiguration),
    SetDefaultSweepTemplate(SetDefaultSweepTemplate),
}
