use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::read_model::ConnectorImportReadModel;

#[derive(Debug, Error)]
pub enum ConnectorImportRepositoryError {
    #[error("connector import repository error: {0}")]
    Internal(String),
}

pub struct ConnectorImportRecord {
    pub connector_id: Uuid,
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub first_imported_at: String,
    pub last_imported_at: String,
    pub latest_sync_id: Option<Uuid>,
}

#[async_trait]
pub trait ConnectorImportRepository: Send + Sync {
    async fn upsert(
        &self,
        record: ConnectorImportRecord,
    ) -> Result<(), ConnectorImportRepositoryError>;

    async fn list_for_documents(
        &self,
        document_ids: &[Uuid],
    ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError>;

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError>;

    async fn document_ids_for_connectors(
        &self,
        connector_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, ConnectorImportRepositoryError>;

    async fn facets(&self) -> Result<Vec<(Uuid, u64)>, ConnectorImportRepositoryError>;

    async fn find(
        &self,
        connector_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<ConnectorImportReadModel>, ConnectorImportRepositoryError>;
}

impl From<ConnectorImportRepositoryError> for ProjectionError {
    fn from(value: ConnectorImportRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
