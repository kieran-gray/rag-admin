use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::Aggregate;

use crate::server::domain::shared::Timestamp;

use super::commands::ConnectorImportCommand;
use super::events::{
    ConnectorImportCompleted, ConnectorImportEvent, ConnectorImportFailed, ConnectorImportStarted,
    ImportItemFailed, ImportItemImported,
};
use super::exceptions::ConnectorImportError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportItemStatus {
    Pending,
    Imported,
    Failed,
}

impl ImportItemStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, ImportItemStatus::Imported | ImportItemStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportItem {
    pub source_ref_key: String,
    pub status: ImportItemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorImport {
    pub import_id: Uuid,
    pub connector_id: Uuid,
    pub index_after_import: bool,
    pub items: Vec<ImportItem>,
    pub status: ImportStatus,
    pub started_at: Timestamp,
}

impl ConnectorImport {
    pub fn item(&self, source_ref_key: &str) -> Option<&ImportItem> {
        self.items
            .iter()
            .find(|i| i.source_ref_key == source_ref_key)
    }

    fn set_item_status(&mut self, source_ref_key: &str, status: ImportItemStatus) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.source_ref_key == source_ref_key)
        {
            item.status = status;
        }
    }

    fn count(&self, status: ImportItemStatus) -> u32 {
        self.items.iter().filter(|i| i.status == status).count() as u32
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
            ConnectorImportEvent::ConnectorImportStarted(_) => {}
            ConnectorImportEvent::ImportItemImported(e) => {
                self.set_item_status(&e.source_ref_key, ImportItemStatus::Imported);
            }
            ConnectorImportEvent::ImportItemFailed(e) => {
                self.set_item_status(&e.source_ref_key, ImportItemStatus::Failed);
            }
            ConnectorImportEvent::ConnectorImportCompleted(_) => {
                self.status = ImportStatus::Completed;
            }
            ConnectorImportEvent::ConnectorImportFailed(_) => {
                self.status = ImportStatus::Failed;
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ConnectorImportCommand::StartConnectorImport(cmd) => {
                if state.is_some() {
                    return Err(ConnectorImportError::AlreadyStarted);
                }
                if cmd.source_ref_keys.is_empty() {
                    return Err(ConnectorImportError::InvalidCommand(
                        "no items selected for import".into(),
                    ));
                }
                Ok(vec![ConnectorImportEvent::ConnectorImportStarted(
                    ConnectorImportStarted {
                        import_id: cmd.import_id,
                        connector_id: cmd.connector_id,
                        source_ref_keys: cmd.source_ref_keys,
                        index_after_import: cmd.index_after_import,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorImportCommand::RecordItemImported(cmd) => {
                let s = running(state)?;
                if s.item(&cmd.source_ref_key).is_none() {
                    return Err(ConnectorImportError::InvalidCommand(format!(
                        "item {} not part of import",
                        cmd.source_ref_key
                    )));
                }
                Ok(vec![ConnectorImportEvent::ImportItemImported(
                    ImportItemImported {
                        import_id: cmd.import_id,
                        source_ref_key: cmd.source_ref_key,
                        document_id: cmd.document_id,
                        document_version: cmd.document_version,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorImportCommand::RecordItemFailed(cmd) => {
                let s = running(state)?;
                if s.item(&cmd.source_ref_key).is_none() {
                    return Err(ConnectorImportError::InvalidCommand(format!(
                        "item {} not part of import",
                        cmd.source_ref_key
                    )));
                }
                Ok(vec![ConnectorImportEvent::ImportItemFailed(
                    ImportItemFailed {
                        import_id: cmd.import_id,
                        source_ref_key: cmd.source_ref_key,
                        error: cmd.error,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorImportCommand::CompleteConnectorImport(cmd) => {
                let s = running(state)?;
                Ok(vec![ConnectorImportEvent::ConnectorImportCompleted(
                    ConnectorImportCompleted {
                        import_id: cmd.import_id,
                        imported: s.count(ImportItemStatus::Imported),
                        failed: s.count(ImportItemStatus::Failed),
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorImportCommand::FailConnectorImport(cmd) => {
                running(state)?;
                Ok(vec![ConnectorImportEvent::ConnectorImportFailed(
                    ConnectorImportFailed {
                        import_id: cmd.import_id,
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
                (None, ConnectorImportEvent::ConnectorImportStarted(e)) => {
                    state = Some(Self {
                        import_id: e.import_id,
                        connector_id: e.connector_id,
                        index_after_import: e.index_after_import,
                        items: e
                            .source_ref_keys
                            .iter()
                            .map(|key| ImportItem {
                                source_ref_key: key.clone(),
                                status: ImportItemStatus::Pending,
                            })
                            .collect(),
                        status: ImportStatus::Running,
                        started_at: e.occurred_at.clone(),
                    });
                }
                (Some(_), ConnectorImportEvent::ConnectorImportStarted(_)) | (None, _) => {
                    return None;
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

fn running(state: Option<&ConnectorImport>) -> Result<&ConnectorImport, ConnectorImportError> {
    let s = state.ok_or(ConnectorImportError::NotFound)?;
    if s.status != ImportStatus::Running {
        return Err(ConnectorImportError::AlreadyTerminal);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::connector_import::commands::{
        CompleteConnectorImport, FailConnectorImport, RecordItemFailed, RecordItemImported,
        StartConnectorImport,
    };

    fn now() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn start(import_id: Uuid, keys: Vec<String>) -> ConnectorImportCommand {
        ConnectorImportCommand::StartConnectorImport(StartConnectorImport {
            import_id,
            connector_id: Uuid::new_v4(),
            source_ref_keys: keys,
            index_after_import: true,
            occurred_at: now(),
        })
    }

    #[test]
    fn start_requires_non_empty_keys() {
        let err = ConnectorImport::handle_command(None, start(Uuid::new_v4(), vec![])).unwrap_err();
        assert!(matches!(err, ConnectorImportError::InvalidCommand(_)));
    }

    #[test]
    fn start_twice_fails() {
        let import_id = Uuid::new_v4();
        let events =
            ConnectorImport::handle_command(None, start(import_id, vec!["url:a".into()])).unwrap();
        let state = ConnectorImport::from_events(&events).unwrap();
        let err =
            ConnectorImport::handle_command(Some(&state), start(import_id, vec!["url:a".into()]))
                .unwrap_err();
        assert!(matches!(err, ConnectorImportError::AlreadyStarted));
    }

    #[test]
    fn happy_path_records_items_and_completes() {
        let import_id = Uuid::new_v4();
        let mut events = ConnectorImport::handle_command(
            None,
            start(import_id, vec!["url:a".into(), "url:b".into()]),
        )
        .unwrap();
        let state = ConnectorImport::from_events(&events).unwrap();
        events.extend(
            ConnectorImport::handle_command(
                Some(&state),
                ConnectorImportCommand::RecordItemImported(RecordItemImported {
                    import_id,
                    source_ref_key: "url:a".into(),
                    document_id: Uuid::new_v4(),
                    document_version: 1,
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorImport::from_events(&events).unwrap();
        events.extend(
            ConnectorImport::handle_command(
                Some(&state),
                ConnectorImportCommand::RecordItemFailed(RecordItemFailed {
                    import_id,
                    source_ref_key: "url:b".into(),
                    error: "boom".into(),
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorImport::from_events(&events).unwrap();
        assert_eq!(
            state.item("url:a").unwrap().status,
            ImportItemStatus::Imported
        );
        assert_eq!(
            state.item("url:b").unwrap().status,
            ImportItemStatus::Failed
        );

        let completed = ConnectorImport::handle_command(
            Some(&state),
            ConnectorImportCommand::CompleteConnectorImport(CompleteConnectorImport {
                import_id,
                occurred_at: now(),
            }),
        )
        .unwrap();
        let ConnectorImportEvent::ConnectorImportCompleted(c) = &completed[0] else {
            panic!("expected completed");
        };
        assert_eq!(c.imported, 1);
        assert_eq!(c.failed, 1);
    }

    #[test]
    fn record_unknown_item_fails() {
        let import_id = Uuid::new_v4();
        let events =
            ConnectorImport::handle_command(None, start(import_id, vec!["url:a".into()])).unwrap();
        let state = ConnectorImport::from_events(&events).unwrap();
        let err = ConnectorImport::handle_command(
            Some(&state),
            ConnectorImportCommand::RecordItemImported(RecordItemImported {
                import_id,
                source_ref_key: "url:missing".into(),
                document_id: Uuid::new_v4(),
                document_version: 1,
                occurred_at: now(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorImportError::InvalidCommand(_)));
    }

    #[test]
    fn record_after_terminal_fails() {
        let import_id = Uuid::new_v4();
        let mut events =
            ConnectorImport::handle_command(None, start(import_id, vec!["url:a".into()])).unwrap();
        let state = ConnectorImport::from_events(&events).unwrap();
        events.extend(
            ConnectorImport::handle_command(
                Some(&state),
                ConnectorImportCommand::FailConnectorImport(FailConnectorImport {
                    import_id,
                    error: "fatal".into(),
                    occurred_at: now(),
                }),
            )
            .unwrap(),
        );
        let state = ConnectorImport::from_events(&events).unwrap();
        assert_eq!(state.status, ImportStatus::Failed);
        let err = ConnectorImport::handle_command(
            Some(&state),
            ConnectorImportCommand::CompleteConnectorImport(CompleteConnectorImport {
                import_id,
                occurred_at: now(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorImportError::AlreadyTerminal));
    }
}
