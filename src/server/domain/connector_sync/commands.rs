use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredItemRecord {
    pub source_ref_key: String,
    pub title: String,
}

pub struct StartConnectorSync {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct RecordDiscoveredItems {
    pub sync_id: Uuid,
    pub items: Vec<DiscoveredItemRecord>,
    pub occurred_at: Timestamp,
}

pub struct CompleteConnectorSync {
    pub sync_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct FailConnectorSync {
    pub sync_id: Uuid,
    pub error: String,
    pub occurred_at: Timestamp,
}

pub enum ConnectorSyncCommand {
    StartConnectorSync(StartConnectorSync),
    RecordDiscoveredItems(RecordDiscoveredItems),
    CompleteConnectorSync(CompleteConnectorSync),
    FailConnectorSync(FailConnectorSync),
}

impl ConnectorSyncCommand {
    pub fn sync_id(&self) -> Uuid {
        match self {
            ConnectorSyncCommand::StartConnectorSync(c) => c.sync_id,
            ConnectorSyncCommand::RecordDiscoveredItems(c) => c.sync_id,
            ConnectorSyncCommand::CompleteConnectorSync(c) => c.sync_id,
            ConnectorSyncCommand::FailConnectorSync(c) => c.sync_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    #[test]
    fn sync_id_returns_start_sync_id() {
        let id = Uuid::new_v4();
        let cmd = ConnectorSyncCommand::StartConnectorSync(StartConnectorSync {
            sync_id: id,
            connector_id: Uuid::nil(),
            occurred_at: ts(),
        });
        assert_eq!(cmd.sync_id(), id);
    }

    #[test]
    fn sync_id_returns_record_items_sync_id() {
        let id = Uuid::new_v4();
        let cmd = ConnectorSyncCommand::RecordDiscoveredItems(RecordDiscoveredItems {
            sync_id: id,
            items: vec![DiscoveredItemRecord {
                source_ref_key: "url:x".into(),
                title: "x".into(),
            }],
            occurred_at: ts(),
        });
        assert_eq!(cmd.sync_id(), id);
    }

    #[test]
    fn sync_id_returns_complete_sync_id() {
        let id = Uuid::new_v4();
        let cmd = ConnectorSyncCommand::CompleteConnectorSync(CompleteConnectorSync {
            sync_id: id,
            occurred_at: ts(),
        });
        assert_eq!(cmd.sync_id(), id);
    }

    #[test]
    fn sync_id_returns_fail_sync_id() {
        let id = Uuid::new_v4();
        let cmd = ConnectorSyncCommand::FailConnectorSync(FailConnectorSync {
            sync_id: id,
            error: "x".into(),
            occurred_at: ts(),
        });
        assert_eq!(cmd.sync_id(), id);
    }
}
