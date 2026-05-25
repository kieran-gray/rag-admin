use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::AiProviderKind;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddRerankerModel {
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateRerankerModel {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveRerankerModel {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RerankerModelCatalogCommand {
    AddRerankerModel(AddRerankerModel),
    UpdateRerankerModel(UpdateRerankerModel),
    RemoveRerankerModel(RemoveRerankerModel),
}
