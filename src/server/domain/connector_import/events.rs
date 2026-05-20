use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorImportRecorded {
    pub connector_id: Uuid,
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub sync_id: Option<Uuid>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorImportEvent {
    ConnectorImportRecorded(ConnectorImportRecorded),
}
