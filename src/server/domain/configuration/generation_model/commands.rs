use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;
use crate::contracts::GenerationModelCommandDto;

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

impl GenerationModelCatalogCommand {
    pub fn from_dto(dto: GenerationModelCommandDto) -> Self {
        match dto {
            GenerationModelCommandDto::AddGenerationModel(d) => {
                Self::AddGenerationModel(AddGenerationModel {
                    kind: d.kind,
                    model: d.model,
                })
            }
            GenerationModelCommandDto::UpdateGenerationModel(d) => {
                Self::UpdateGenerationModel(UpdateGenerationModel {
                    model_id: d.model_id,
                    kind: d.kind,
                    model: d.model,
                })
            }
            GenerationModelCommandDto::RemoveGenerationModel(d) => {
                Self::RemoveGenerationModel(RemoveGenerationModel {
                    model_id: d.model_id,
                })
            }
        }
    }
}
