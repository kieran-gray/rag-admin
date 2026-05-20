use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use crate::server::domain::shared::Timestamp;

use super::commands::ConnectorSyncCommand;
use super::events::{
    ConnectorItemsDiscovered, ConnectorSyncCompleted, ConnectorSyncEvent, ConnectorSyncFailed,
    ConnectorSyncStarted,
};
use super::exceptions::ConnectorSyncError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSync {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub status: SyncStatus,
    pub discovered_count: u32,
    pub started_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub error: Option<String>,
}

impl Aggregate for ConnectorSync {
    type Event = ConnectorSyncEvent;
    type Command = ConnectorSyncCommand;
    type Error = ConnectorSyncError;

    fn aggregate_type() -> &'static str {
        "connector_sync"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ConnectorSyncEvent::ConnectorSyncStarted(_) => {}
            ConnectorSyncEvent::ConnectorItemsDiscovered(e) => {
                self.discovered_count = self.discovered_count.saturating_add(e.items.len() as u32);
            }
            ConnectorSyncEvent::ConnectorSyncCompleted(e) => {
                self.status = SyncStatus::Completed;
                self.completed_at = Some(e.occurred_at.clone());
            }
            ConnectorSyncEvent::ConnectorSyncFailed(e) => {
                self.status = SyncStatus::Failed;
                self.completed_at = Some(e.occurred_at.clone());
                self.error = Some(e.error.clone());
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ConnectorSyncCommand::StartConnectorSync(cmd) => {
                if state.is_some() {
                    return Err(ConnectorSyncError::AlreadyStarted);
                }
                Ok(vec![ConnectorSyncEvent::ConnectorSyncStarted(
                    ConnectorSyncStarted {
                        sync_id: cmd.sync_id,
                        connector_id: cmd.connector_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorSyncCommand::RecordDiscoveredItems(cmd) => {
                let s = state.ok_or(ConnectorSyncError::NotFound)?;
                if s.status != SyncStatus::Started {
                    return Err(ConnectorSyncError::AlreadyTerminal);
                }
                Ok(vec![ConnectorSyncEvent::ConnectorItemsDiscovered(
                    ConnectorItemsDiscovered {
                        sync_id: cmd.sync_id,
                        items: cmd.items,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorSyncCommand::CompleteConnectorSync(cmd) => {
                let s = state.ok_or(ConnectorSyncError::NotFound)?;
                if s.status != SyncStatus::Started {
                    return Err(ConnectorSyncError::AlreadyTerminal);
                }
                Ok(vec![ConnectorSyncEvent::ConnectorSyncCompleted(
                    ConnectorSyncCompleted {
                        sync_id: cmd.sync_id,
                        discovered_count: s.discovered_count,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorSyncCommand::FailConnectorSync(cmd) => {
                let s = state.ok_or(ConnectorSyncError::NotFound)?;
                if s.status != SyncStatus::Started {
                    return Err(ConnectorSyncError::AlreadyTerminal);
                }
                Ok(vec![ConnectorSyncEvent::ConnectorSyncFailed(
                    ConnectorSyncFailed {
                        sync_id: cmd.sync_id,
                        error: cmd.error,
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
                (None, ConnectorSyncEvent::ConnectorSyncStarted(e)) => {
                    state = Some(Self {
                        sync_id: e.sync_id,
                        connector_id: e.connector_id,
                        status: SyncStatus::Started,
                        discovered_count: 0,
                        started_at: e.occurred_at.clone(),
                        completed_at: None,
                        error: None,
                    });
                }
                (Some(_), ConnectorSyncEvent::ConnectorSyncStarted(_)) | (None, _) => {
                    return None;
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<ConnectorSync, ()> for ConnectorSyncEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::connector_sync::commands::{
        CompleteConnectorSync, DiscoveredItemRecord, FailConnectorSync, RecordDiscoveredItems,
        StartConnectorSync,
    };

    fn now() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn start(sync_id: Uuid, connector_id: Uuid) -> ConnectorSyncCommand {
        ConnectorSyncCommand::StartConnectorSync(StartConnectorSync {
            sync_id,
            connector_id,
            occurred_at: now(),
        })
    }

    #[test]
    fn happy_path_emits_started_discovered_completed() {
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        let mut events = ConnectorSync::handle_command(None, start(sync_id, connector_id)).unwrap();
        let state = ConnectorSync::from_events(&events).unwrap();
        events.extend(
            ConnectorSync::handle_command(
                Some(&state),
                ConnectorSyncCommand::RecordDiscoveredItems(RecordDiscoveredItems {
                    sync_id,
                    items: vec![DiscoveredItemRecord {
                        source_ref_key: "url:a".into(),
                        title: "a".into(),
                    }],
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorSync::from_events(&events).unwrap();
        events.extend(
            ConnectorSync::handle_command(
                Some(&state),
                ConnectorSyncCommand::CompleteConnectorSync(CompleteConnectorSync {
                    sync_id,
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorSync::from_events(&events).unwrap();
        assert_eq!(state.status, SyncStatus::Completed);
        assert_eq!(state.discovered_count, 1);
    }

    #[test]
    fn fail_after_started_marks_failed() {
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        let mut events = ConnectorSync::handle_command(None, start(sync_id, connector_id)).unwrap();
        let state = ConnectorSync::from_events(&events).unwrap();
        events.extend(
            ConnectorSync::handle_command(
                Some(&state),
                ConnectorSyncCommand::FailConnectorSync(FailConnectorSync {
                    sync_id,
                    error: "boom".into(),
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorSync::from_events(&events).unwrap();
        assert_eq!(state.status, SyncStatus::Failed);
        assert_eq!(state.error.as_deref(), Some("boom"));
    }

    #[test]
    fn record_items_after_completed_fails() {
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        let mut events = ConnectorSync::handle_command(None, start(sync_id, connector_id)).unwrap();
        let state = ConnectorSync::from_events(&events).unwrap();
        events.extend(
            ConnectorSync::handle_command(
                Some(&state),
                ConnectorSyncCommand::CompleteConnectorSync(CompleteConnectorSync {
                    sync_id,
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorSync::from_events(&events).unwrap();
        let err = ConnectorSync::handle_command(
            Some(&state),
            ConnectorSyncCommand::RecordDiscoveredItems(RecordDiscoveredItems {
                sync_id,
                items: vec![],
                occurred_at: now(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorSyncError::AlreadyTerminal));
    }
}
