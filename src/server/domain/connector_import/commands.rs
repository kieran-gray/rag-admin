use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

pub struct RecordConnectorImport {
    pub connector_id: Uuid,
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub sync_id: Option<Uuid>,
    pub occurred_at: Timestamp,
}

pub enum ConnectorImportCommand {
    RecordConnectorImport(RecordConnectorImport),
}
