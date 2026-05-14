use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::ChunkingConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKindDto {
    Cloudflare,
    Ollama,
}

impl AiProviderKindDto {
    pub fn as_str(self) -> &'static str {
        match self {
            AiProviderKindDto::Cloudflare => "cloudflare",
            AiProviderKindDto::Ollama => "ollama",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            AiProviderKindDto::Cloudflare => "Cloudflare",
            AiProviderKindDto::Ollama => "Ollama",
        }
    }

    pub fn all() -> &'static [AiProviderKindDto] {
        &[AiProviderKindDto::Cloudflare, AiProviderKindDto::Ollama]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreKindDto {
    CloudflareVectorize,
    Postgres,
}

impl VectorStoreKindDto {
    pub fn as_str(self) -> &'static str {
        match self {
            VectorStoreKindDto::CloudflareVectorize => "cloudflare_vectorize",
            VectorStoreKindDto::Postgres => "postgres",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            VectorStoreKindDto::CloudflareVectorize => "Cloudflare Vectorize",
            VectorStoreKindDto::Postgres => "Postgres (pgvector)",
        }
    }

    pub fn all() -> &'static [VectorStoreKindDto] {
        &[
            VectorStoreKindDto::CloudflareVectorize,
            VectorStoreKindDto::Postgres,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingModelDto {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKindDto,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationModelDto {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKindDto,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorIndexDto {
    pub index_id: Uuid,
    pub kind: VectorStoreKindDto,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigurationDto {
    pub configuration_id: Uuid,
    pub embedding_models: Vec<EmbeddingModelDto>,
    pub generation_models: Vec<GenerationModelDto>,
    pub vector_indexes: Vec<VectorIndexDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineConfigurationDto {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub embedding_model_name: Option<String>,
    pub generation_model_id: Uuid,
    pub generation_model_name: Option<String>,
    pub vector_index_id: Uuid,
    pub vector_index_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingConfigurationDto {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SweepTemplateDto {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
    pub is_default: bool,
}
