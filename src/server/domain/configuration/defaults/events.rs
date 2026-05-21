use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultChunkingConfigurationSet {
    pub chunking_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultIndexProfileSet {
    pub index_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultRetrievalProfileSet {
    pub retrieval_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultSweepTemplateSet {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum ConfigurationDefaultsEvent {
    DefaultChunkingConfigurationSet(DefaultChunkingConfigurationSet),
    DefaultIndexProfileSet(DefaultIndexProfileSet),
    DefaultRetrievalProfileSet(DefaultRetrievalProfileSet),
    DefaultSweepTemplateSet(DefaultSweepTemplateSet),
}
