use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Resolved references the command handler captures from the catalogs at
/// command-construction time. The aggregate trusts these snapshots: it
/// validates dimensions agree, but does not look the entries up itself.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbeddingModelRef {
    pub embedding_model_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VectorIndexRef {
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GenerationModelRef {
    pub generation_model_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddPipelineConfiguration {
    pub name: String,
    pub embedding_model: EmbeddingModelRef,
    pub generation_model: GenerationModelRef,
    pub vector_index: VectorIndexRef,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdatePipelineConfiguration {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model: EmbeddingModelRef,
    pub generation_model: GenerationModelRef,
    pub vector_index: VectorIndexRef,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemovePipelineConfiguration {
    pub pipeline_configuration_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PipelineConfigurationCatalogCommand {
    AddPipelineConfiguration(AddPipelineConfiguration),
    UpdatePipelineConfiguration(UpdatePipelineConfiguration),
    RemovePipelineConfiguration(RemovePipelineConfiguration),
}
