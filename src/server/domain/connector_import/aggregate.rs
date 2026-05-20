use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use crate::server::domain::shared::Timestamp;

use super::commands::ConnectorImportCommand;
use super::events::{ConnectorImportEvent, ConnectorImportRecorded};
use super::exceptions::ConnectorImportError;

const CONNECTOR_IMPORT_NAMESPACE: Uuid = uuid::uuid!("d3b1c5a4-7f9e-4a82-9d77-2e3a8b4c5d61");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorImport {
    pub connector_id: Uuid,
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub first_imported_at: Timestamp,
    pub last_imported_at: Timestamp,
    pub latest_sync_id: Option<Uuid>,
}

impl ConnectorImport {
    pub fn compute_id(connector_id: Uuid, document_id: Uuid) -> Uuid {
        let name = format!("{connector_id}:{document_id}");
        Uuid::new_v5(&CONNECTOR_IMPORT_NAMESPACE, name.as_bytes())
    }

    fn from_recorded(e: &ConnectorImportRecorded) -> Self {
        Self {
            connector_id: e.connector_id,
            document_id: e.document_id,
            source_ref_key: e.source_ref_key.clone(),
            first_imported_at: e.occurred_at.clone(),
            last_imported_at: e.occurred_at.clone(),
            latest_sync_id: e.sync_id,
        }
    }
}

impl Aggregate for ConnectorImport {
    type Event = ConnectorImportEvent;
    type Command = ConnectorImportCommand;
    type Error = ConnectorImportError;

    fn aggregate_type() -> &'static str {
        "connector_import"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ConnectorImportEvent::ConnectorImportRecorded(e) => {
                self.last_imported_at = e.occurred_at.clone();
                if e.sync_id.is_some() {
                    self.latest_sync_id = e.sync_id;
                }
                self.source_ref_key.clone_from(&e.source_ref_key);
            }
        }
    }

    fn handle_command(
        _state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ConnectorImportCommand::RecordConnectorImport(cmd) => {
                Ok(vec![ConnectorImportEvent::ConnectorImportRecorded(
                    ConnectorImportRecorded {
                        connector_id: cmd.connector_id,
                        document_id: cmd.document_id,
                        source_ref_key: cmd.source_ref_key,
                        sync_id: cmd.sync_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, ConnectorImportEvent::ConnectorImportRecorded(e)) => {
                    state = Some(Self::from_recorded(e));
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<ConnectorImport, ()> for ConnectorImportEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::connector_import::commands::RecordConnectorImport;

    fn now() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn later() -> Timestamp {
        "2024-02-01T00:00:00Z".into()
    }

    fn cmd(connector_id: Uuid, document_id: Uuid, when: Timestamp) -> ConnectorImportCommand {
        ConnectorImportCommand::RecordConnectorImport(RecordConnectorImport {
            connector_id,
            document_id,
            source_ref_key: "url:https://example.com/a".into(),
            sync_id: None,
            occurred_at: when,
        })
    }

    #[test]
    fn first_import_initializes_state() {
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let events = ConnectorImport::handle_command(None, cmd(c, d, now())).unwrap();
        assert_eq!(events.len(), 1);
        let state = ConnectorImport::from_events(&events).unwrap();
        assert_eq!(state.connector_id, c);
        assert_eq!(state.document_id, d);
        assert_eq!(state.first_imported_at, now());
        assert_eq!(state.last_imported_at, now());
    }

    #[test]
    fn re_import_preserves_first_and_updates_last() {
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let mut events = ConnectorImport::handle_command(None, cmd(c, d, now())).unwrap();
        let state = ConnectorImport::from_events(&events).unwrap();
        events.extend(ConnectorImport::handle_command(Some(&state), cmd(c, d, later())).unwrap());
        let state = ConnectorImport::from_events(&events).unwrap();
        assert_eq!(state.first_imported_at, now());
        assert_eq!(state.last_imported_at, later());
    }

    #[test]
    fn compute_id_is_deterministic_per_pair() {
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let id1 = ConnectorImport::compute_id(c, d);
        let id2 = ConnectorImport::compute_id(c, d);
        assert_eq!(id1, id2);
        let other = ConnectorImport::compute_id(Uuid::new_v4(), d);
        assert_ne!(id1, other);
    }

    #[test]
    fn manual_reimport_without_sync_id_preserves_latest_sync_id() {
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let sync_id = Uuid::new_v4();
        let bulk_events = vec![ConnectorImportEvent::ConnectorImportRecorded(
            ConnectorImportRecorded {
                connector_id: c,
                document_id: d,
                source_ref_key: "url:x".into(),
                sync_id: Some(sync_id),
                occurred_at: now(),
            },
        )];
        let state = ConnectorImport::from_events(&bulk_events).unwrap();
        assert_eq!(state.latest_sync_id, Some(sync_id));

        let mut all = bulk_events;
        all.extend(ConnectorImport::handle_command(Some(&state), cmd(c, d, later())).unwrap());
        let restate = ConnectorImport::from_events(&all).unwrap();
        assert_eq!(restate.latest_sync_id, Some(sync_id));
        assert_eq!(restate.last_imported_at, later());
    }

    #[test]
    fn reimport_with_new_sync_id_overwrites_previous() {
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let mut events = vec![ConnectorImportEvent::ConnectorImportRecorded(
            ConnectorImportRecorded {
                connector_id: c,
                document_id: d,
                source_ref_key: "url:x".into(),
                sync_id: Some(first),
                occurred_at: now(),
            },
        )];
        events.push(ConnectorImportEvent::ConnectorImportRecorded(
            ConnectorImportRecorded {
                connector_id: c,
                document_id: d,
                source_ref_key: "url:x".into(),
                sync_id: Some(second),
                occurred_at: later(),
            },
        ));
        let state = ConnectorImport::from_events(&events).unwrap();
        assert_eq!(state.latest_sync_id, Some(second));
    }
}
