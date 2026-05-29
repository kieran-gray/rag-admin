use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

pub struct StartConnectorImport {
    pub import_id: Uuid,
    pub connector_id: Uuid,
    pub source_ref_keys: Vec<String>,
    pub index_after_import: bool,
    pub occurred_at: Timestamp,
}

pub struct RecordItemImported {
    pub import_id: Uuid,
    pub source_ref_key: String,
    pub document_id: Uuid,
    pub document_version: u32,
    pub occurred_at: Timestamp,
}

pub struct RecordItemFailed {
    pub import_id: Uuid,
    pub source_ref_key: String,
    pub error: String,
    pub occurred_at: Timestamp,
}

pub struct CompleteConnectorImport {
    pub import_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct FailConnectorImport {
    pub import_id: Uuid,
    pub error: String,
    pub occurred_at: Timestamp,
}

pub enum ConnectorImportCommand {
    StartConnectorImport(StartConnectorImport),
    RecordItemImported(RecordItemImported),
    RecordItemFailed(RecordItemFailed),
    CompleteConnectorImport(CompleteConnectorImport),
    FailConnectorImport(FailConnectorImport),
}

impl ConnectorImportCommand {
    pub fn import_id(&self) -> Uuid {
        match self {
            ConnectorImportCommand::StartConnectorImport(c) => c.import_id,
            ConnectorImportCommand::RecordItemImported(c) => c.import_id,
            ConnectorImportCommand::RecordItemFailed(c) => c.import_id,
            ConnectorImportCommand::CompleteConnectorImport(c) => c.import_id,
            ConnectorImportCommand::FailConnectorImport(c) => c.import_id,
        }
    }
}
