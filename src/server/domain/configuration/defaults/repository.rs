use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::read_model::ConfigurationDefaultsReadModel;

#[derive(Debug, Error)]
pub enum ConfigurationDefaultsRepositoryError {
    #[error("configuration defaults repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait ConfigurationDefaultsRepository: Send + Sync {
    async fn load(
        &self,
    ) -> Result<ConfigurationDefaultsReadModel, ConfigurationDefaultsRepositoryError>;

    async fn set_chunking_configuration(&self, id: Uuid) -> Result<(), ProjectionError>;

    async fn set_pipeline_configuration(&self, id: Uuid) -> Result<(), ProjectionError>;
}

impl From<ConfigurationDefaultsRepositoryError> for ProjectionError {
    fn from(value: ConfigurationDefaultsRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
