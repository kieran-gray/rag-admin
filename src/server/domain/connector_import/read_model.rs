use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorImportSummary {
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
