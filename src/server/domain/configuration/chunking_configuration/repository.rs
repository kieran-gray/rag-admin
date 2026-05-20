use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use event_sourcing::error::ProjectionError;

use super::entity::ChunkingConfiguration;
use super::read_model::ChunkingConfigurationReadModel;

#[derive(Debug, Error)]
pub enum ChunkingConfigurationRepositoryError {
    #[error("chunking configuration repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait ChunkingConfigurationRepository:
    CatalogRepository<ChunkingConfiguration> + Send + Sync
{
    async fn load_all(
        &self,
    ) -> Result<Vec<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError>;
}

impl From<ChunkingConfigurationRepositoryError> for ProjectionError {
    fn from(value: ChunkingConfigurationRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
