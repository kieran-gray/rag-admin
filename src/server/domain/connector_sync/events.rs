use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::commands::DiscoveredItemRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorSyncStarted {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorItemsDiscovered {
    pub sync_id: Uuid,
    pub items: Vec<DiscoveredItemRecord>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorSyncCompleted {
    pub sync_id: Uuid,
    pub discovered_count: u32,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorSyncFailed {
    pub sync_id: Uuid,
    pub error: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorSyncEvent {
    ConnectorSyncStarted(ConnectorSyncStarted),
    ConnectorItemsDiscovered(ConnectorItemsDiscovered),
    ConnectorSyncCompleted(ConnectorSyncCompleted),
    ConnectorSyncFailed(ConnectorSyncFailed),
}
