use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{AiProviderKindDto, ChunkingConfig, VectorStoreKindDto};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEmbeddingModelDto {
    pub kind: AiProviderKindDto,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEmbeddingModelDto {
    pub model_id: Uuid,
    pub kind: AiProviderKindDto,
    pub model: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveEmbeddingModelDto {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGenerationModelDto {
    pub kind: AiProviderKindDto,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGenerationModelDto {
    pub model_id: Uuid,
    pub kind: AiProviderKindDto,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveGenerationModelDto {
    pub model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddVectorIndexDto {
    pub kind: VectorStoreKindDto,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateVectorIndexDto {
    pub index_id: Uuid,
    pub kind: VectorStoreKindDto,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveVectorIndexDto {
    pub index_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineConfigurationDto {
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePipelineConfigurationDto {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePipelineConfigurationDto {
    pub pipeline_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChunkingConfigurationDto {
    pub name: String,
    pub config: ChunkingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChunkingConfigurationDto {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
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
pub enum VectorIndexCommandDto {
    AddVectorIndex(AddVectorIndexDto),
    UpdateVectorIndex(UpdateVectorIndexDto),
    RemoveVectorIndex(RemoveVectorIndexDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PipelineConfigurationCommandDto {
    CreatePipelineConfiguration(CreatePipelineConfigurationDto),
    UpdatePipelineConfiguration(UpdatePipelineConfigurationDto),
    DeletePipelineConfiguration(DeletePipelineConfigurationDto),
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
