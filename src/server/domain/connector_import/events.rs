use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorImportStarted {
    pub import_id: Uuid,
    pub connector_id: Uuid,
    pub source_ref_keys: Vec<String>,
    pub index_after_import: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportItemImported {
    pub import_id: Uuid,
    pub source_ref_key: String,
    pub document_id: Uuid,
    pub document_version: u32,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportItemFailed {
    pub import_id: Uuid,
    pub source_ref_key: String,
    pub error: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorImportCompleted {
    pub import_id: Uuid,
    pub imported: u32,
    pub failed: u32,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorImportFailed {
    pub import_id: Uuid,
    pub error: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorImportEvent {
    ConnectorImportStarted(ConnectorImportStarted),
    ImportItemImported(ImportItemImported),
    ImportItemFailed(ImportItemFailed),
    ConnectorImportCompleted(ConnectorImportCompleted),
    ConnectorImportFailed(ConnectorImportFailed),
}
