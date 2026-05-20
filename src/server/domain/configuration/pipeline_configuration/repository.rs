use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use event_sourcing::error::ProjectionError;

use super::entity::PipelineConfiguration;
use super::read_model::PipelineConfigurationReadModel;

#[derive(Debug, Error)]
pub enum PipelineConfigurationRepositoryError {
    #[error("pipeline configuration repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait PipelineConfigurationRepository:
    CatalogRepository<PipelineConfiguration> + Send + Sync
{
    async fn load_all(
        &self,
    ) -> Result<Vec<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError>;
}

impl From<PipelineConfigurationRepositoryError> for ProjectionError {
    fn from(value: PipelineConfigurationRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
