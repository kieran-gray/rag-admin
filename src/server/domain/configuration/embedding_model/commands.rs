use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;
use crate::shared::EmbeddingModelCommandDto;

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
        use crate::server::domain::configuration::kinds::AiProviderKind as Kind;
        use crate::shared::AiProviderKindDto;
        let kind = |k: AiProviderKindDto| match k {
            AiProviderKindDto::Cloudflare => Kind::Cloudflare,
            AiProviderKindDto::Ollama => Kind::Ollama,
        };
        match dto {
            EmbeddingModelCommandDto::AddEmbeddingModel(d) => {
                Self::AddEmbeddingModel(AddEmbeddingModel {
                    kind: kind(d.kind),
                    model: d.model,
                    dimensions: d.dimensions,
                })
            }
            EmbeddingModelCommandDto::UpdateEmbeddingModel(d) => {
                Self::UpdateEmbeddingModel(UpdateEmbeddingModel {
                    model_id: d.model_id,
                    kind: kind(d.kind),
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
