use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::read_model::{ConnectorDiscoveredItemReadModel, ConnectorSyncSummary};

#[derive(Debug, Error)]
pub enum ConnectorSyncRepositoryError {
    #[error("connector sync repository error: {0}")]
    Internal(String),
}

pub struct ConnectorSyncRecord {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub status: String,
    pub discovered_count: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[async_trait]
pub trait ConnectorSyncRepository: Send + Sync {
    async fn upsert(&self, record: ConnectorSyncRecord)
        -> Result<(), ConnectorSyncRepositoryError>;

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConnectorSyncSummary>, ConnectorSyncRepositoryError>;

    async fn latest_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError>;

    async fn find_by_sync_id(
        &self,
        sync_id: Uuid,
    ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError>;
}

impl From<ConnectorSyncRepositoryError> for ProjectionError {
    fn from(value: ConnectorSyncRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ConnectorDiscoveredItemRepositoryError {
    #[error("connector discovered item repository error: {0}")]
    Internal(String),
}

pub struct DiscoveredItemUpsert {
    pub connector_id: Uuid,
    pub source_ref_key: String,
    pub title: String,
    pub seen_at: String,
    pub sync_id: Uuid,
}

#[async_trait]
pub trait ConnectorDiscoveredItemRepository: Send + Sync {
    async fn upsert(
        &self,
        item: DiscoveredItemUpsert,
    ) -> Result<(), ConnectorDiscoveredItemRepositoryError>;

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorDiscoveredItemReadModel>, ConnectorDiscoveredItemRepositoryError>;
}

impl From<ConnectorDiscoveredItemRepositoryError> for ProjectionError {
    fn from(value: ConnectorDiscoveredItemRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
