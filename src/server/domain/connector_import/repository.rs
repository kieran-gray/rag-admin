use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::read_model::ConnectorImportSummary;

#[derive(Debug, Error)]
pub enum ConnectorImportRepositoryError {
    #[error("connector import repository error: {0}")]
    Internal(String),
}

pub struct ConnectorImportRecord {
    pub import_id: Uuid,
    pub connector_id: Uuid,
    pub status: String,
    pub total: u32,
    pub imported: u32,
    pub failed: u32,
    pub index_after_import: bool,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[async_trait]
pub trait ConnectorImportRepository: Send + Sync {
    async fn upsert(
        &self,
        record: ConnectorImportRecord,
    ) -> Result<(), ConnectorImportRepositoryError>;

    async fn find_by_import_id(
        &self,
        import_id: Uuid,
    ) -> Result<Option<ConnectorImportSummary>, ConnectorImportRepositoryError>;

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConnectorImportSummary>, ConnectorImportRepositoryError>;
}

impl From<ConnectorImportRepositoryError> for ProjectionError {
    fn from(value: ConnectorImportRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
