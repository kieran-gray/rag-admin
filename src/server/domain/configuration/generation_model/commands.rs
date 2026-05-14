use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;
use crate::shared::GenerationModelCommandDto;

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
        use crate::server::domain::configuration::kinds::AiProviderKind as Kind;
        use crate::shared::AiProviderKindDto;
        let kind = |k: AiProviderKindDto| match k {
            AiProviderKindDto::Cloudflare => Kind::Cloudflare,
            AiProviderKindDto::Ollama => Kind::Ollama,
        };
        match dto {
            GenerationModelCommandDto::AddGenerationModel(d) => {
                Self::AddGenerationModel(AddGenerationModel {
                    kind: kind(d.kind),
                    model: d.model,
                })
            }
            GenerationModelCommandDto::UpdateGenerationModel(d) => {
                Self::UpdateGenerationModel(UpdateGenerationModel {
                    model_id: d.model_id,
                    kind: kind(d.kind),
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
