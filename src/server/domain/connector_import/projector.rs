use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConnectorImportEvent;
use super::read_model::ConnectorImportSummary;
use super::repository::{ConnectorImportRecord, ConnectorImportRepository};

pub struct ConnectorImportProjector {
    repository: Arc<dyn ConnectorImportRepository>,
}

impl ConnectorImportProjector {
    pub const NAME: &'static str = "connector_import_projector";

    pub fn new(repository: Arc<dyn ConnectorImportRepository>) -> Self {
        Self { repository }
    }

    async fn load(&self, import_id: uuid::Uuid) -> Result<ConnectorImportSummary, ProjectionError> {
        self.repository
            .find_by_import_id(import_id)
            .await?
            .ok_or_else(|| {
                ProjectionError::Storage(format!("connector import {import_id} not found"))
            })
    }
}

fn record_from(summary: ConnectorImportSummary) -> ConnectorImportRecord {
    ConnectorImportRecord {
        import_id: summary.import_id,
        connector_id: summary.connector_id,
        status: summary.status,
        total: summary.total,
        imported: summary.imported,
        failed: summary.failed,
        index_after_import: summary.index_after_import,
        started_at: summary.started_at,
        completed_at: summary.completed_at,
        error: summary.error,
    }
}

#[async_trait]
impl Projector<ConnectorImportEvent> for ConnectorImportProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ConnectorImportEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            match &envelope.event {
                ConnectorImportEvent::ConnectorImportStarted(e) => {
                    self.repository
                        .upsert(ConnectorImportRecord {
                            import_id: e.import_id,
                            connector_id: e.connector_id,
                            status: "running".into(),
                            total: e.source_ref_keys.len() as u32,
                            imported: 0,
                            failed: 0,
                            index_after_import: e.index_after_import,
                            started_at: e.occurred_at.to_string(),
                            completed_at: None,
                            error: None,
                        })
                        .await?;
                }
                ConnectorImportEvent::ImportItemImported(e) => {
                    let mut summary = self.load(e.import_id).await?;
                    summary.imported = summary.imported.saturating_add(1);
                    self.repository.upsert(record_from(summary)).await?;
                }
                ConnectorImportEvent::ImportItemFailed(e) => {
                    let mut summary = self.load(e.import_id).await?;
                    summary.failed = summary.failed.saturating_add(1);
                    self.repository.upsert(record_from(summary)).await?;
                }
                ConnectorImportEvent::ConnectorImportCompleted(e) => {
                    let mut summary = self.load(e.import_id).await?;
                    summary.status = "completed".into();
                    summary.imported = e.imported;
                    summary.failed = e.failed;
                    summary.completed_at = Some(e.occurred_at.to_string());
                    self.repository.upsert(record_from(summary)).await?;
                }
                ConnectorImportEvent::ConnectorImportFailed(e) => {
                    let mut summary = self.load(e.import_id).await?;
                    summary.status = "failed".into();
                    summary.error = Some(e.error.clone());
                    summary.completed_at = Some(e.occurred_at.to_string());
                    self.repository.upsert(record_from(summary)).await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::Mutex;

    use event_sourcing::envelope::EventMetadata;
    use uuid::Uuid;

    use crate::server::domain::connector_import::events::{
        ConnectorImportCompleted, ConnectorImportFailed, ConnectorImportStarted, ImportItemFailed,
        ImportItemImported,
    };
    use crate::server::domain::connector_import::repository::ConnectorImportRepositoryError;

    #[derive(Default)]
    struct InMemRepo {
        rows: Mutex<HashMap<Uuid, ConnectorImportSummary>>,
    }

    impl InMemRepo {
        fn get(&self, import_id: Uuid) -> Option<ConnectorImportSummary> {
            self.rows.lock().expect("lock").get(&import_id).cloned()
        }
    }

    #[async_trait]
    impl ConnectorImportRepository for InMemRepo {
        async fn upsert(
            &self,
            record: ConnectorImportRecord,
        ) -> Result<(), ConnectorImportRepositoryError> {
            self.rows.lock().expect("lock").insert(
                record.import_id,
                ConnectorImportSummary {
                    import_id: record.import_id,
                    connector_id: record.connector_id,
                    status: record.status,
                    total: record.total,
                    imported: record.imported,
                    failed: record.failed,
                    index_after_import: record.index_after_import,
                    started_at: record.started_at,
                    completed_at: record.completed_at,
                    error: record.error,
                },
            );
            Ok(())
        }

        async fn find_by_import_id(
            &self,
            import_id: Uuid,
        ) -> Result<Option<ConnectorImportSummary>, ConnectorImportRepositoryError> {
            Ok(self.get(import_id))
        }

        async fn list_for_connector(
            &self,
            _connector_id: Uuid,
            _limit: u32,
            _offset: u32,
        ) -> Result<Vec<ConnectorImportSummary>, ConnectorImportRepositoryError> {
            Ok(Vec::new())
        }
    }

    fn envelope(
        stream_id: Uuid,
        event: ConnectorImportEvent,
        log_position: i64,
    ) -> EventEnvelope<ConnectorImportEvent> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id,
                aggregate_type: "connector_import".into(),
                sequence: log_position,
                log_position,
                event_type: "Ev".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn started(import_id: Uuid, connector_id: Uuid, keys: usize) -> ConnectorImportEvent {
        ConnectorImportEvent::ConnectorImportStarted(ConnectorImportStarted {
            import_id,
            connector_id,
            source_ref_keys: (0..keys).map(|i| format!("url:{i}")).collect(),
            index_after_import: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })
    }

    #[tokio::test]
    async fn started_creates_running_row_with_total() {
        let repo = Arc::new(InMemRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let import_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[envelope(import_id, started(import_id, connector_id, 3), 1)])
            .await
            .expect("project");

        let summary = repo.get(import_id).expect("present");
        assert_eq!(summary.status, "running");
        assert_eq!(summary.total, 3);
        assert_eq!(summary.imported, 0);
        assert_eq!(summary.failed, 0);
        assert!(summary.index_after_import);
    }

    #[tokio::test]
    async fn item_events_increment_running_counts() {
        let repo = Arc::new(InMemRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let import_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                envelope(import_id, started(import_id, connector_id, 2), 1),
                envelope(
                    import_id,
                    ConnectorImportEvent::ImportItemImported(ImportItemImported {
                        import_id,
                        source_ref_key: "url:0".into(),
                        document_id: Uuid::new_v4(),
                        document_version: 1,
                        occurred_at: "t".into(),
                    }),
                    2,
                ),
                envelope(
                    import_id,
                    ConnectorImportEvent::ImportItemFailed(ImportItemFailed {
                        import_id,
                        source_ref_key: "url:1".into(),
                        error: "boom".into(),
                        occurred_at: "t".into(),
                    }),
                    3,
                ),
            ])
            .await
            .expect("project");

        let summary = repo.get(import_id).unwrap();
        assert_eq!(summary.imported, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.status, "running");
    }

    #[tokio::test]
    async fn completed_sets_status_and_final_counts() {
        let repo = Arc::new(InMemRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let import_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                envelope(import_id, started(import_id, connector_id, 2), 1),
                envelope(
                    import_id,
                    ConnectorImportEvent::ConnectorImportCompleted(ConnectorImportCompleted {
                        import_id,
                        imported: 2,
                        failed: 0,
                        occurred_at: "2024-02-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        let summary = repo.get(import_id).unwrap();
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.imported, 2);
        assert!(summary.completed_at.is_some());
    }

    #[tokio::test]
    async fn failed_sets_status_and_error() {
        let repo = Arc::new(InMemRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let import_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                envelope(import_id, started(import_id, connector_id, 1), 1),
                envelope(
                    import_id,
                    ConnectorImportEvent::ConnectorImportFailed(ConnectorImportFailed {
                        import_id,
                        error: "fatal".into(),
                        occurred_at: "2024-02-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        let summary = repo.get(import_id).unwrap();
        assert_eq!(summary.status, "failed");
        assert_eq!(summary.error.as_deref(), Some("fatal"));
    }

    #[tokio::test]
    async fn item_event_for_unknown_import_errors() {
        let repo = Arc::new(InMemRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let import_id = Uuid::new_v4();
        let result = projector
            .project(&[envelope(
                import_id,
                ConnectorImportEvent::ImportItemImported(ImportItemImported {
                    import_id,
                    source_ref_key: "url:0".into(),
                    document_id: Uuid::new_v4(),
                    document_version: 1,
                    occurred_at: "t".into(),
                }),
                1,
            )])
            .await;
        assert!(result.is_err());
    }
}
