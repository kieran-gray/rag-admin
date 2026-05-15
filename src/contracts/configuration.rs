use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::{AiProviderKind, VectorStoreKind};
use crate::core::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingModelDto {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationModelDto {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorIndexDto {
    pub index_id: Uuid,
    pub kind: VectorStoreKind,
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
