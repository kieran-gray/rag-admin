use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenerationModelCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenerationModelAdded {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenerationModelUpdated {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenerationModelRemoved {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum GenerationModelCatalogEvent {
    GenerationModelCatalogCreated(GenerationModelCatalogCreated),
    GenerationModelAdded(GenerationModelAdded),
    GenerationModelUpdated(GenerationModelUpdated),
    GenerationModelRemoved(GenerationModelRemoved),
}
