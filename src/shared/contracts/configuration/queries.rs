use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};
use crate::shared::ChunkingConfig;

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
pub struct RerankerModelDto {
    pub reranker_model_id: Uuid,
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
    pub reranker_models: Vec<RerankerModelDto>,
    pub vector_indexes: Vec<VectorIndexDto>,
    pub index_profiles: Vec<IndexProfileDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexProfileDto {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub embedding_model_name: Option<String>,
    pub vector_index_id: Uuid,
    pub vector_index_name: Option<String>,
    pub dimensions: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievalProfileDto {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile_id: Uuid,
    pub index_profile_name: Option<String>,
    pub generation_model_id: Uuid,
    pub generation_model_name: Option<String>,
    pub reranker_model_id: Option<Uuid>,
    pub reranker_model_name: Option<String>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingConfigurationDto {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SweepTemplateDto {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
    pub is_default: bool,
}
