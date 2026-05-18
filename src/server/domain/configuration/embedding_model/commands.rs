use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::AiProviderKind;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddEmbeddingModel {
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateEmbeddingModel {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveEmbeddingModel {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EmbeddingModelCatalogCommand {
    AddEmbeddingModel(AddEmbeddingModel),
    UpdateEmbeddingModel(UpdateEmbeddingModel),
    RemoveEmbeddingModel(RemoveEmbeddingModel),
}
