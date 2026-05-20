use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorImportReadModel {
    pub connector_id: Uuid,
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub first_imported_at: String,
    pub last_imported_at: String,
    pub latest_sync_id: Option<Uuid>,
}
