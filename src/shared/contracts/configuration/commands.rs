use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};
use crate::shared::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEmbeddingModelDto {
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEmbeddingModelDto {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveEmbeddingModelDto {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGenerationModelDto {
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGenerationModelDto {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveGenerationModelDto {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRerankerModelDto {
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRerankerModelDto {
    pub model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveRerankerModelDto {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddVectorIndexDto {
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateVectorIndexDto {
    pub index_id: Uuid,
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveVectorIndexDto {
    pub index_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIndexProfileDto {
    pub name: String,
    pub embedding_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIndexProfileDto {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteIndexProfileDto {
    pub index_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRetrievalProfileDto {
    pub name: String,
    pub index_profile_id: Uuid,
    pub generation_model_id: Uuid,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRetrievalProfileDto {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile_id: Uuid,
    pub generation_model_id: Uuid,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRetrievalProfileDto {
    pub retrieval_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChunkingConfigurationDto {
    pub name: String,
    pub config: ChunkingConfig,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChunkingConfigurationDto {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteChunkingConfigurationDto {
    pub chunking_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSweepTemplateDto {
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSweepTemplateDto {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSweepTemplateDto {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultSweepTemplateDto {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EmbeddingModelCommandDto {
    AddEmbeddingModel(AddEmbeddingModelDto),
    UpdateEmbeddingModel(UpdateEmbeddingModelDto),
    RemoveEmbeddingModel(RemoveEmbeddingModelDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum GenerationModelCommandDto {
    AddGenerationModel(AddGenerationModelDto),
    UpdateGenerationModel(UpdateGenerationModelDto),
    RemoveGenerationModel(RemoveGenerationModelDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RerankerModelCommandDto {
    AddRerankerModel(AddRerankerModelDto),
    UpdateRerankerModel(UpdateRerankerModelDto),
    RemoveRerankerModel(RemoveRerankerModelDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VectorIndexCommandDto {
    AddVectorIndex(AddVectorIndexDto),
    UpdateVectorIndex(UpdateVectorIndexDto),
    RemoveVectorIndex(RemoveVectorIndexDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IndexProfileCommandDto {
    CreateIndexProfile(CreateIndexProfileDto),
    UpdateIndexProfile(UpdateIndexProfileDto),
    DeleteIndexProfile(DeleteIndexProfileDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RetrievalProfileCommandDto {
    CreateRetrievalProfile(CreateRetrievalProfileDto),
    UpdateRetrievalProfile(UpdateRetrievalProfileDto),
    DeleteRetrievalProfile(DeleteRetrievalProfileDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ChunkingConfigurationCommandDto {
    CreateChunkingConfiguration(CreateChunkingConfigurationDto),
    UpdateChunkingConfiguration(UpdateChunkingConfigurationDto),
    DeleteChunkingConfiguration(DeleteChunkingConfigurationDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SweepTemplateCommandDto {
    CreateSweepTemplate(CreateSweepTemplateDto),
    UpdateSweepTemplate(UpdateSweepTemplateDto),
    DeleteSweepTemplate(DeleteSweepTemplateDto),
    SetDefaultSweepTemplate(SetDefaultSweepTemplateDto),
}
