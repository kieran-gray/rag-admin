use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::shared::ChunkingConfig;

use super::read_model::ChunkingConfigurationReadModel;

#[derive(Debug, Error)]
pub enum ChunkingConfigurationRepositoryError {
    #[error("chunking configuration not found: {0}")]
    NotFound(Uuid),
    #[error("chunking configuration with this name already exists")]
    NameConflict,
    #[error("referenced generation model not found: {0}")]
    ReferenceViolation(String),
    #[error("chunking configuration repository error: {0}")]
    Internal(String),
}

pub struct NewChunkingConfiguration {
    pub id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

pub struct ChunkingConfigurationUpdate {
    pub id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

#[async_trait]
pub trait ChunkingConfigurationRepository: Send + Sync {
    async fn load_all(
        &self,
    ) -> Result<Vec<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError>;

    async fn create(
        &self,
        row: NewChunkingConfiguration,
    ) -> Result<(), ChunkingConfigurationRepositoryError>;

    async fn update(
        &self,
        row: ChunkingConfigurationUpdate,
    ) -> Result<(), ChunkingConfigurationRepositoryError>;

    async fn delete(&self, id: Uuid) -> Result<(), ChunkingConfigurationRepositoryError>;
}
