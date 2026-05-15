use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;
use crate::contracts::EmbeddingModelCommandDto;

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

impl EmbeddingModelCatalogCommand {
    pub fn from_dto(dto: EmbeddingModelCommandDto) -> Self {
        match dto {
            EmbeddingModelCommandDto::AddEmbeddingModel(d) => {
                Self::AddEmbeddingModel(AddEmbeddingModel {
                    kind: d.kind,
                    model: d.model,
                    dimensions: d.dimensions,
                })
            }
            EmbeddingModelCommandDto::UpdateEmbeddingModel(d) => {
                Self::UpdateEmbeddingModel(UpdateEmbeddingModel {
                    model_id: d.model_id,
                    kind: d.kind,
                    model: d.model,
                    dimensions: d.dimensions,
                })
            }
            EmbeddingModelCommandDto::RemoveEmbeddingModel(d) => {
                Self::RemoveEmbeddingModel(RemoveEmbeddingModel {
                    model_id: d.model_id,
                })
            }
        }
    }
}
