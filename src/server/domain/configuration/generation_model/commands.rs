use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddGenerationModel {
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateGenerationModel {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveGenerationModel {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum GenerationModelCatalogCommand {
    AddGenerationModel(AddGenerationModel),
    UpdateGenerationModel(UpdateGenerationModel),
    RemoveGenerationModel(RemoveGenerationModel),
}
