use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConnectorImportEvent;
use super::repository::{ConnectorImportRecord, ConnectorImportRepository};

pub struct ConnectorImportProjector {
    repository: Arc<dyn ConnectorImportRepository>,
}

impl ConnectorImportProjector {
    pub const NAME: &'static str = "connector_import_projector";

    pub fn new(repository: Arc<dyn ConnectorImportRepository>) -> Self {
        Self { repository }
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
                ConnectorImportEvent::ConnectorImportRecorded(e) => {
                    let occurred_at = e.occurred_at.to_string();
                    let existing = self.repository.find(e.connector_id, e.document_id).await?;
                    let first_imported_at = existing
                        .as_ref()
                        .map(|m| m.first_imported_at.clone())
                        .unwrap_or_else(|| occurred_at.clone());
                    let latest_sync_id = e
                        .sync_id
                        .or_else(|| existing.as_ref().and_then(|m| m.latest_sync_id));
                    self.repository
                        .upsert(ConnectorImportRecord {
                            connector_id: e.connector_id,
                            document_id: e.document_id,
                            source_ref_key: e.source_ref_key.clone(),
                            first_imported_at,
                            last_imported_at: occurred_at,
                            latest_sync_id,
                        })
                        .await?;
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

    use crate::server::domain::connector_import::events::ConnectorImportRecorded;
    use crate::server::domain::connector_import::read_model::ConnectorImportReadModel;
    use crate::server::domain::connector_import::repository::ConnectorImportRepositoryError;

    #[derive(Default)]
    struct InMemImportRepo {
        records: Mutex<HashMap<(Uuid, Uuid), ConnectorImportReadModel>>,
    }

    #[async_trait]
    impl ConnectorImportRepository for InMemImportRepo {
        async fn upsert(
            &self,
            record: ConnectorImportRecord,
        ) -> Result<(), ConnectorImportRepositoryError> {
            let key = (record.connector_id, record.document_id);
            let read_model = ConnectorImportReadModel {
                connector_id: record.connector_id,
                document_id: record.document_id,
                source_ref_key: record.source_ref_key,
                first_imported_at: record.first_imported_at,
                last_imported_at: record.last_imported_at,
                latest_sync_id: record.latest_sync_id,
            };
            self.records.lock().expect("lock").insert(key, read_model);
            Ok(())
        }

        async fn list_for_documents(
            &self,
            _document_ids: &[Uuid],
        ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_for_connector(
            &self,
            _connector_id: Uuid,
        ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
            Ok(Vec::new())
        }

        async fn document_ids_for_connectors(
            &self,
            _connector_ids: &[Uuid],
        ) -> Result<Vec<Uuid>, ConnectorImportRepositoryError> {
            Ok(Vec::new())
        }

        async fn facets(&self) -> Result<Vec<(Uuid, u64)>, ConnectorImportRepositoryError> {
            Ok(Vec::new())
        }

        async fn find(
            &self,
            connector_id: Uuid,
            document_id: Uuid,
        ) -> Result<Option<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
            Ok(self
                .records
                .lock()
                .expect("lock")
                .get(&(connector_id, document_id))
                .cloned())
        }
    }

    fn envelope(
        connector_id: Uuid,
        document_id: Uuid,
        sync_id: Option<Uuid>,
        occurred_at: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorImportEvent> {
        EventEnvelope {
            event: ConnectorImportEvent::ConnectorImportRecorded(ConnectorImportRecorded {
                connector_id,
                document_id,
                source_ref_key: "url:x".into(),
                sync_id,
                occurred_at: occurred_at.into(),
            }),
            metadata: EventMetadata {
                stream_id: Uuid::new_v4(),
                aggregate_type: "connector_import".into(),
                sequence: 1,
                log_position,
                event_type: "ConnectorImportRecorded".into(),
                occurred_at: occurred_at.into(),
            },
        }
    }

    #[tokio::test]
    async fn first_event_initialises_first_and_last_to_event_time() {
        let repo = Arc::new(InMemImportRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();

        projector
            .project(&[envelope(
                c,
                d,
                Some(Uuid::new_v4()),
                "2024-01-01T00:00:00Z",
                1,
            )])
            .await
            .expect("project");

        let stored = repo.find(c, d).await.expect("find").expect("present");
        assert_eq!(stored.first_imported_at, "2024-01-01T00:00:00Z");
        assert_eq!(stored.last_imported_at, "2024-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn re_import_updates_last_but_preserves_first() {
        let repo = Arc::new(InMemImportRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();

        projector
            .project(&[envelope(c, d, None, "2024-01-01T00:00:00Z", 1)])
            .await
            .expect("project");
        projector
            .project(&[envelope(c, d, None, "2024-02-15T00:00:00Z", 2)])
            .await
            .expect("project");

        let stored = repo.find(c, d).await.expect("find").expect("present");
        assert_eq!(stored.first_imported_at, "2024-01-01T00:00:00Z");
        assert_eq!(stored.last_imported_at, "2024-02-15T00:00:00Z");
    }

    #[tokio::test]
    async fn manual_reimport_without_sync_id_preserves_latest_sync_id() {
        let repo = Arc::new(InMemImportRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let bulk_sync = Uuid::new_v4();

        projector
            .project(&[envelope(c, d, Some(bulk_sync), "2024-01-01T00:00:00Z", 1)])
            .await
            .expect("project");
        projector
            .project(&[envelope(c, d, None, "2024-02-15T00:00:00Z", 2)])
            .await
            .expect("project");

        let stored = repo.find(c, d).await.expect("find").expect("present");
        assert_eq!(stored.latest_sync_id, Some(bulk_sync));
    }

    #[tokio::test]
    async fn reimport_with_new_sync_id_replaces_previous() {
        let repo = Arc::new(InMemImportRepo::default());
        let projector = ConnectorImportProjector::new(repo.clone());
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        projector
            .project(&[
                envelope(c, d, Some(first), "2024-01-01T00:00:00Z", 1),
                envelope(c, d, Some(second), "2024-02-15T00:00:00Z", 2),
            ])
            .await
            .expect("project");

        let stored = repo.find(c, d).await.expect("find").expect("present");
        assert_eq!(stored.latest_sync_id, Some(second));
    }
}
