use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::PipelineConfigurationReadModel;

#[derive(Debug, Error)]
pub enum PipelineConfigurationRepositoryError {
    #[error("pipeline configuration not found: {0}")]
    NotFound(Uuid),
    #[error("pipeline configuration with this name already exists")]
    NameConflict,
    #[error("referenced embedding model, generation model, or vector index not found, or embedding/index dimensions do not match: {0}")]
    ReferenceViolation(String),
    #[error("pipeline configuration repository error: {0}")]
    Internal(String),
}

pub struct NewPipelineConfiguration {
    pub id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
}

pub struct PipelineConfigurationUpdate {
    pub id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
}

#[async_trait]
pub trait PipelineConfigurationRepository: Send + Sync {
    async fn load_all(
        &self,
    ) -> Result<Vec<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError>;

    async fn create(
        &self,
        row: NewPipelineConfiguration,
    ) -> Result<(), PipelineConfigurationRepositoryError>;

    async fn update(
        &self,
        row: PipelineConfigurationUpdate,
    ) -> Result<(), PipelineConfigurationRepositoryError>;

    async fn delete(&self, id: Uuid) -> Result<(), PipelineConfigurationRepositoryError>;
}
