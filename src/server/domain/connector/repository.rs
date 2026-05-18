use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::read_model::ConnectorReadModel;

#[derive(Debug, Error)]
pub enum ConnectorRepositoryError {
    #[error("connector repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait ConnectorRepository: Send + Sync {
    async fn load(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorReadModel>, ConnectorRepositoryError>;

    async fn save(&self, read_model: ConnectorReadModel) -> Result<(), ConnectorRepositoryError>;

    async fn mark_deleted(
        &self,
        connector_id: Uuid,
        updated_at: String,
    ) -> Result<(), ConnectorRepositoryError>;

    async fn list_active(&self) -> Result<Vec<ConnectorReadModel>, ConnectorRepositoryError>;
}

impl From<ConnectorRepositoryError> for ProjectionError {
    fn from(value: ConnectorRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
