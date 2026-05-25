use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::AiProviderKind;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RerankerModelCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RerankerModelAdded {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RerankerModelUpdated {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RerankerModelRemoved {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum RerankerModelCatalogEvent {
    RerankerModelCatalogCreated(RerankerModelCatalogCreated),
    RerankerModelAdded(RerankerModelAdded),
    RerankerModelUpdated(RerankerModelUpdated),
    RerankerModelRemoved(RerankerModelRemoved),
}
