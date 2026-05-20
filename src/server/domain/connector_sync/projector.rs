use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConnectorSyncEvent;
use super::repository::{
    ConnectorDiscoveredItemRepository, ConnectorSyncRecord, ConnectorSyncRepository,
    DiscoveredItemUpsert,
};

pub struct ConnectorSyncProjector {
    sync_repository: Arc<dyn ConnectorSyncRepository>,
    discovered_repository: Arc<dyn ConnectorDiscoveredItemRepository>,
}

impl ConnectorSyncProjector {
    pub const NAME: &'static str = "connector_sync_projector";

    pub fn new(
        sync_repository: Arc<dyn ConnectorSyncRepository>,
        discovered_repository: Arc<dyn ConnectorDiscoveredItemRepository>,
    ) -> Self {
        Self {
            sync_repository,
            discovered_repository,
        }
    }
}

#[async_trait]
impl Projector<ConnectorSyncEvent> for ConnectorSyncProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ConnectorSyncEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            match &envelope.event {
                ConnectorSyncEvent::ConnectorSyncStarted(e) => {
                    self.sync_repository
                        .upsert(ConnectorSyncRecord {
                            sync_id: e.sync_id,
                            connector_id: e.connector_id,
                            status: "started".into(),
                            discovered_count: 0,
                            started_at: e.occurred_at.to_string(),
                            completed_at: None,
                            error: None,
                        })
                        .await?;
                }
                ConnectorSyncEvent::ConnectorItemsDiscovered(e) => {
                    let mut summary = self
                        .sync_repository
                        .find_by_sync_id(e.sync_id)
                        .await?
                        .ok_or_else(|| {
                            ProjectionError::Storage(format!(
                                "sync {} not found while projecting discovered items",
                                e.sync_id
                            ))
                        })?;
                    let connector_id = summary.connector_id;
                    for item in &e.items {
                        self.discovered_repository
                            .upsert(DiscoveredItemUpsert {
                                connector_id,
                                source_ref_key: item.source_ref_key.clone(),
                                title: item.title.clone(),
                                seen_at: e.occurred_at.to_string(),
                                sync_id: e.sync_id,
                            })
                            .await?;
                    }
                    summary.discovered_count = summary
                        .discovered_count
                        .saturating_add(e.items.len() as u32);
                    self.sync_repository
                        .upsert(ConnectorSyncRecord {
                            sync_id: summary.sync_id,
                            connector_id: summary.connector_id,
                            status: summary.status,
                            discovered_count: summary.discovered_count,
                            started_at: summary.started_at,
                            completed_at: summary.completed_at,
                            error: summary.error,
                        })
                        .await?;
                }
                ConnectorSyncEvent::ConnectorSyncCompleted(e) => {
                    let s = self
                        .sync_repository
                        .find_by_sync_id(e.sync_id)
                        .await?
                        .ok_or_else(|| {
                            ProjectionError::Storage(format!(
                                "sync {} not found while projecting completion",
                                e.sync_id
                            ))
                        })?;
                    self.sync_repository
                        .upsert(ConnectorSyncRecord {
                            sync_id: s.sync_id,
                            connector_id: s.connector_id,
                            status: "completed".into(),
                            discovered_count: e.discovered_count,
                            started_at: s.started_at,
                            completed_at: Some(e.occurred_at.to_string()),
                            error: None,
                        })
                        .await?;
                }
                ConnectorSyncEvent::ConnectorSyncFailed(e) => {
                    let s = self
                        .sync_repository
                        .find_by_sync_id(e.sync_id)
                        .await?
                        .ok_or_else(|| {
                            ProjectionError::Storage(format!(
                                "sync {} not found while projecting failure",
                                e.sync_id
                            ))
                        })?;
                    self.sync_repository
                        .upsert(ConnectorSyncRecord {
                            sync_id: s.sync_id,
                            connector_id: s.connector_id,
                            status: "failed".into(),
                            discovered_count: s.discovered_count,
                            started_at: s.started_at,
                            completed_at: Some(e.occurred_at.to_string()),
                            error: Some(e.error.clone()),
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

    use crate::server::domain::connector_sync::commands::DiscoveredItemRecord;
    use crate::server::domain::connector_sync::events::{
        ConnectorItemsDiscovered, ConnectorSyncCompleted, ConnectorSyncFailed, ConnectorSyncStarted,
    };
    use crate::server::domain::connector_sync::read_model::{
        ConnectorDiscoveredItemReadModel, ConnectorSyncSummary,
    };
    use crate::server::domain::connector_sync::repository::{
        ConnectorDiscoveredItemRepositoryError, ConnectorSyncRepositoryError,
    };

    #[derive(Default)]
    struct InMemSyncRepo {
        records: Mutex<HashMap<Uuid, ConnectorSyncSummary>>,
        fail_next_find: Mutex<bool>,
    }

    impl InMemSyncRepo {
        fn get(&self, sync_id: Uuid) -> Option<ConnectorSyncSummary> {
            self.records.lock().expect("lock").get(&sync_id).cloned()
        }
    }

    #[async_trait]
    impl ConnectorSyncRepository for InMemSyncRepo {
        async fn upsert(
            &self,
            record: ConnectorSyncRecord,
        ) -> Result<(), ConnectorSyncRepositoryError> {
            self.records.lock().expect("lock").insert(
                record.sync_id,
                ConnectorSyncSummary {
                    sync_id: record.sync_id,
                    connector_id: record.connector_id,
                    status: record.status,
                    discovered_count: record.discovered_count,
                    started_at: record.started_at,
                    completed_at: record.completed_at,
                    error: record.error,
                },
            );
            Ok(())
        }

        async fn list_for_connector(
            &self,
            _connector_id: Uuid,
            _limit: u32,
        ) -> Result<Vec<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
            Ok(Vec::new())
        }

        async fn latest_for_connector(
            &self,
            _connector_id: Uuid,
        ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
            Ok(None)
        }

        async fn find_by_sync_id(
            &self,
            sync_id: Uuid,
        ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
            if *self.fail_next_find.lock().expect("lock") {
                *self.fail_next_find.lock().expect("lock") = false;
                return Err(ConnectorSyncRepositoryError::Internal("forced".into()));
            }
            Ok(self.records.lock().expect("lock").get(&sync_id).cloned())
        }
    }

    #[derive(Default)]
    struct InMemDiscoveredRepo {
        items: Mutex<Vec<DiscoveredItemUpsert>>,
    }

    impl InMemDiscoveredRepo {
        fn count(&self) -> usize {
            self.items.lock().expect("lock").len()
        }

        fn keys(&self) -> Vec<String> {
            self.items
                .lock()
                .expect("lock")
                .iter()
                .map(|i| i.source_ref_key.clone())
                .collect()
        }

        fn connector_ids(&self) -> Vec<Uuid> {
            self.items
                .lock()
                .expect("lock")
                .iter()
                .map(|i| i.connector_id)
                .collect()
        }
    }

    #[async_trait]
    impl ConnectorDiscoveredItemRepository for InMemDiscoveredRepo {
        async fn upsert(
            &self,
            item: DiscoveredItemUpsert,
        ) -> Result<(), ConnectorDiscoveredItemRepositoryError> {
            self.items.lock().expect("lock").push(item);
            Ok(())
        }

        async fn list_for_connector(
            &self,
            _connector_id: Uuid,
        ) -> Result<Vec<ConnectorDiscoveredItemReadModel>, ConnectorDiscoveredItemRepositoryError>
        {
            Ok(Vec::new())
        }
    }

    fn envelope<T>(stream_id: Uuid, event: T, log_position: i64) -> EventEnvelope<T> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id,
                aggregate_type: "connector_sync".into(),
                sequence: log_position,
                log_position,
                event_type: "Ev".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn started(
        sync_id: Uuid,
        connector_id: Uuid,
        log_position: i64,
    ) -> EventEnvelope<ConnectorSyncEvent> {
        envelope(
            sync_id,
            ConnectorSyncEvent::ConnectorSyncStarted(ConnectorSyncStarted {
                sync_id,
                connector_id,
                occurred_at: "2024-01-01T00:00:00Z".into(),
            }),
            log_position,
        )
    }

    fn discovered(
        sync_id: Uuid,
        items: Vec<(&str, &str)>,
        log_position: i64,
        when: &str,
    ) -> EventEnvelope<ConnectorSyncEvent> {
        envelope(
            sync_id,
            ConnectorSyncEvent::ConnectorItemsDiscovered(ConnectorItemsDiscovered {
                sync_id,
                items: items
                    .into_iter()
                    .map(|(k, t)| DiscoveredItemRecord {
                        source_ref_key: k.into(),
                        title: t.into(),
                    })
                    .collect(),
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    fn completed(
        sync_id: Uuid,
        count: u32,
        log_position: i64,
    ) -> EventEnvelope<ConnectorSyncEvent> {
        envelope(
            sync_id,
            ConnectorSyncEvent::ConnectorSyncCompleted(ConnectorSyncCompleted {
                sync_id,
                discovered_count: count,
                occurred_at: "2024-02-01T00:00:00Z".into(),
            }),
            log_position,
        )
    }

    fn failed(sync_id: Uuid, error: &str, log_position: i64) -> EventEnvelope<ConnectorSyncEvent> {
        envelope(
            sync_id,
            ConnectorSyncEvent::ConnectorSyncFailed(ConnectorSyncFailed {
                sync_id,
                error: error.into(),
                occurred_at: "2024-02-01T00:00:00Z".into(),
            }),
            log_position,
        )
    }

    fn projector_with() -> (
        ConnectorSyncProjector,
        Arc<InMemSyncRepo>,
        Arc<InMemDiscoveredRepo>,
    ) {
        let sync_repo = Arc::new(InMemSyncRepo::default());
        let discovered_repo = Arc::new(InMemDiscoveredRepo::default());
        let projector = ConnectorSyncProjector::new(sync_repo.clone(), discovered_repo.clone());
        (projector, sync_repo, discovered_repo)
    }

    #[tokio::test]
    async fn sync_started_creates_summary_with_started_status_and_zero_count() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[started(sync_id, connector_id, 1)])
            .await
            .expect("project");

        let summary = sync_repo.get(sync_id).expect("present");
        assert_eq!(summary.connector_id, connector_id);
        assert_eq!(summary.status, "started");
        assert_eq!(summary.discovered_count, 0);
        assert!(summary.completed_at.is_none());
        assert!(summary.error.is_none());
    }

    #[tokio::test]
    async fn items_discovered_upserts_each_item_with_connector_id_from_summary() {
        let (projector, _, discovered_repo) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                discovered(
                    sync_id,
                    vec![("url:a", "A"), ("url:b", "B")],
                    2,
                    "2024-01-01T00:00:01Z",
                ),
            ])
            .await
            .expect("project");

        assert_eq!(discovered_repo.count(), 2);
        assert_eq!(discovered_repo.keys(), vec!["url:a", "url:b"]);
        for cid in discovered_repo.connector_ids() {
            assert_eq!(cid, connector_id);
        }
    }

    #[tokio::test]
    async fn items_discovered_increments_discovered_count_on_summary() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                discovered(sync_id, vec![("a", "A"), ("b", "B"), ("c", "C")], 2, "t"),
            ])
            .await
            .expect("project");

        assert_eq!(sync_repo.get(sync_id).unwrap().discovered_count, 3);
    }

    #[tokio::test]
    async fn multiple_discovered_batches_accumulate_count() {
        let (projector, sync_repo, discovered_repo) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                discovered(sync_id, vec![("a", "A")], 2, "t"),
                discovered(sync_id, vec![("b", "B"), ("c", "C")], 3, "t"),
            ])
            .await
            .expect("project");

        assert_eq!(sync_repo.get(sync_id).unwrap().discovered_count, 3);
        assert_eq!(discovered_repo.count(), 3);
    }

    #[tokio::test]
    async fn items_discovered_for_unknown_sync_returns_error() {
        let (projector, _, discovered_repo) = projector_with();
        let result = projector
            .project(&[discovered(Uuid::new_v4(), vec![("a", "A")], 1, "t")])
            .await;
        assert!(result.is_err());
        assert_eq!(discovered_repo.count(), 0);
    }

    #[tokio::test]
    async fn sync_completed_marks_status_and_records_completed_at() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[started(sync_id, connector_id, 1), completed(sync_id, 5, 2)])
            .await
            .expect("project");

        let summary = sync_repo.get(sync_id).unwrap();
        assert_eq!(summary.status, "completed");
        assert!(summary.completed_at.is_some());
        assert!(summary.error.is_none());
    }

    #[tokio::test]
    async fn sync_completed_overrides_discovered_count_with_event_value() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                discovered(sync_id, vec![("a", "A"), ("b", "B")], 2, "t"),
                completed(sync_id, 99, 3),
            ])
            .await
            .expect("project");

        assert_eq!(
            sync_repo.get(sync_id).unwrap().discovered_count,
            99,
            "Completed event's count must override the accumulated total",
        );
    }

    #[tokio::test]
    async fn sync_failed_marks_status_and_carries_error_message() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                failed(sync_id, "rate limited", 2),
            ])
            .await
            .expect("project");

        let summary = sync_repo.get(sync_id).unwrap();
        assert_eq!(summary.status, "failed");
        assert_eq!(summary.error.as_deref(), Some("rate limited"));
        assert!(summary.completed_at.is_some());
    }

    #[tokio::test]
    async fn sync_failed_preserves_existing_discovered_count() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();

        projector
            .project(&[
                started(sync_id, connector_id, 1),
                discovered(sync_id, vec![("a", "A"), ("b", "B")], 2, "t"),
                failed(sync_id, "boom", 3),
            ])
            .await
            .expect("project");

        assert_eq!(sync_repo.get(sync_id).unwrap().discovered_count, 2);
    }

    #[tokio::test]
    async fn sync_completed_for_unknown_sync_id_returns_storage_error() {
        let (projector, _, _) = projector_with();
        let err = projector
            .project(&[completed(Uuid::new_v4(), 1, 1)])
            .await
            .expect_err("must error");
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn sync_failed_for_unknown_sync_id_returns_storage_error() {
        let (projector, _, _) = projector_with();
        let err = projector
            .project(&[failed(Uuid::new_v4(), "x", 1)])
            .await
            .expect_err("must error");
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn sync_completed_propagates_repo_find_error() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        projector
            .project(&[started(sync_id, connector_id, 1)])
            .await
            .expect("seed");

        *sync_repo.fail_next_find.lock().expect("lock") = true;
        let err = projector
            .project(&[completed(sync_id, 5, 2)])
            .await
            .expect_err("must propagate");
        assert!(err.to_string().contains("forced"));
        assert_eq!(
            sync_repo.get(sync_id).unwrap().status,
            "started",
            "find error must abort the upsert so status stays unchanged",
        );
    }

    #[tokio::test]
    async fn sync_failed_propagates_repo_find_error() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        projector
            .project(&[started(sync_id, connector_id, 1)])
            .await
            .expect("seed");

        *sync_repo.fail_next_find.lock().expect("lock") = true;
        let err = projector
            .project(&[failed(sync_id, "boom", 2)])
            .await
            .expect_err("must propagate");
        assert!(err.to_string().contains("forced"));
        assert_eq!(sync_repo.get(sync_id).unwrap().status, "started");
    }

    #[tokio::test]
    async fn items_discovered_propagates_repo_find_error_distinctly_from_not_found() {
        let (projector, sync_repo, _) = projector_with();
        let sync_id = Uuid::new_v4();
        let connector_id = Uuid::new_v4();
        projector
            .project(&[started(sync_id, connector_id, 1)])
            .await
            .expect("seed");

        *sync_repo.fail_next_find.lock().expect("lock") = true;
        let err = projector
            .project(&[discovered(sync_id, vec![("a", "A")], 2, "t")])
            .await
            .expect_err("must propagate");
        let msg = err.to_string();
        assert!(
            msg.contains("forced"),
            "original repo error must surface, not the 'not found' fallback: {msg}",
        );
        assert!(!msg.contains("not found"));
    }

    #[tokio::test]
    async fn multi_sync_batch_isolates_per_sync_id() {
        let (projector, sync_repo, discovered_repo) = projector_with();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let ca = Uuid::new_v4();
        let cb = Uuid::new_v4();

        projector
            .project(&[
                started(a, ca, 1),
                started(b, cb, 2),
                discovered(a, vec![("a:1", "A1")], 3, "t"),
                discovered(b, vec![("b:1", "B1"), ("b:2", "B2")], 4, "t"),
                completed(a, 1, 5),
                failed(b, "boom", 6),
            ])
            .await
            .expect("project");

        let sa = sync_repo.get(a).unwrap();
        let sb = sync_repo.get(b).unwrap();
        assert_eq!(sa.status, "completed");
        assert_eq!(sa.discovered_count, 1);
        assert!(sa.error.is_none());
        assert_eq!(sb.status, "failed");
        assert_eq!(sb.discovered_count, 2);
        assert_eq!(sb.error.as_deref(), Some("boom"));

        assert_eq!(discovered_repo.count(), 3);
        let connector_ids = discovered_repo.connector_ids();
        assert_eq!(connector_ids.iter().filter(|c| **c == ca).count(), 1);
        assert_eq!(connector_ids.iter().filter(|c| **c == cb).count(), 2);
    }

    #[tokio::test]
    async fn projector_name_is_stable_constant() {
        let (projector, _, _) = projector_with();
        assert_eq!(projector.name(), ConnectorSyncProjector::NAME);
        assert_eq!(ConnectorSyncProjector::NAME, "connector_sync_projector");
    }
}
