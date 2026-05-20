use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSyncSummary {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub status: String,
    pub discovered_count: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDiscoveredItemReadModel {
    pub connector_id: Uuid,
    pub source_ref_key: String,
    pub title: String,
    pub first_seen: String,
    pub last_seen: String,
    pub latest_sync_id: Uuid,
}
