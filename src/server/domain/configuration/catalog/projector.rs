use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::repository::CatalogRepository;

pub enum CatalogProjectorAction<E> {
    Noop,
    Upsert(E),
    Remove(Uuid),
}

pub struct CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync,
{
    name: &'static str,
    repository: Arc<dyn CatalogRepository<Entry>>,
    classify: fn(&Event) -> CatalogProjectorAction<Entry>,
}

impl<Event, Entry> CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync,
{
    pub fn new(
        name: &'static str,
        repository: Arc<dyn CatalogRepository<Entry>>,
        classify: fn(&Event) -> CatalogProjectorAction<Entry>,
    ) -> Self {
        Self {
            name,
            repository,
            classify,
        }
    }
}

#[async_trait]
impl<Event, Entry> Projector<Event> for CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        self.name
    }

    async fn project(&self, events: &[EventEnvelope<Event>]) -> Result<(), ProjectionError> {
        for envelope in events {
            match (self.classify)(&envelope.event) {
                CatalogProjectorAction::Noop => {}
                CatalogProjectorAction::Upsert(entry) => self.repository.upsert(entry).await?,
                CatalogProjectorAction::Remove(id) => self.repository.delete(id).await?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_sourcing::envelope::EventMetadata;
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    struct Entry {
        id: Uuid,
        name: String,
    }

    enum Op {
        Upsert(Entry),
        Delete(Uuid),
    }

    enum Ev {
        Created,
        Add(Entry),
        Replace(Entry),
        Drop(Uuid),
    }

    #[derive(Default)]
    struct RecordingRepo {
        calls: Mutex<Vec<Op>>,
        fail_next: Mutex<bool>,
    }

    impl RecordingRepo {
        fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .expect("lock")
                .iter()
                .map(|op| match op {
                    Op::Upsert(e) => format!("upsert:{}", e.name),
                    Op::Delete(id) => format!("delete:{id}"),
                })
                .collect()
        }
    }

    #[async_trait]
    impl CatalogRepository<Entry> for RecordingRepo {
        async fn upsert(&self, entry: Entry) -> Result<(), ProjectionError> {
            if *self.fail_next.lock().expect("lock") {
                *self.fail_next.lock().expect("lock") = false;
                return Err(ProjectionError::Storage("forced".into()));
            }
            self.calls.lock().expect("lock").push(Op::Upsert(entry));
            Ok(())
        }

        async fn delete(&self, id: Uuid) -> Result<(), ProjectionError> {
            if *self.fail_next.lock().expect("lock") {
                *self.fail_next.lock().expect("lock") = false;
                return Err(ProjectionError::Storage("forced".into()));
            }
            self.calls.lock().expect("lock").push(Op::Delete(id));
            Ok(())
        }
    }

    fn classify(ev: &Ev) -> CatalogProjectorAction<Entry> {
        match ev {
            Ev::Created => CatalogProjectorAction::Noop,
            Ev::Add(e) | Ev::Replace(e) => CatalogProjectorAction::Upsert(e.clone()),
            Ev::Drop(id) => CatalogProjectorAction::Remove(*id),
        }
    }

    fn envelope(event: Ev, log_position: i64) -> EventEnvelope<Ev> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id: Uuid::nil(),
                aggregate_type: "test".into(),
                sequence: log_position,
                log_position,
                event_type: "x".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    #[tokio::test]
    async fn noop_events_make_no_calls_to_repository() {
        let repo = Arc::new(RecordingRepo::default());
        let projector = CatalogProjector::new("p", repo.clone(), classify);
        projector
            .project(&[envelope(Ev::Created, 1)])
            .await
            .expect("project");
        assert!(repo.calls().is_empty());
    }

    #[tokio::test]
    async fn upsert_action_forwards_entry_to_repository() {
        let repo = Arc::new(RecordingRepo::default());
        let projector = CatalogProjector::new("p", repo.clone(), classify);
        let entry = Entry {
            id: Uuid::new_v4(),
            name: "a".into(),
        };
        projector
            .project(&[envelope(Ev::Add(entry.clone()), 1)])
            .await
            .expect("project");
        assert_eq!(repo.calls(), vec!["upsert:a"]);
    }

    #[tokio::test]
    async fn remove_action_forwards_id_to_repository() {
        let repo = Arc::new(RecordingRepo::default());
        let projector = CatalogProjector::new("p", repo.clone(), classify);
        let id = Uuid::new_v4();
        projector
            .project(&[envelope(Ev::Drop(id), 1)])
            .await
            .expect("project");
        assert_eq!(repo.calls(), vec![format!("delete:{id}")]);
    }

    #[tokio::test]
    async fn upserts_and_removes_are_applied_in_envelope_order() {
        let repo = Arc::new(RecordingRepo::default());
        let projector = CatalogProjector::new("p", repo.clone(), classify);
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        projector
            .project(&[
                envelope(Ev::Created, 1),
                envelope(
                    Ev::Add(Entry {
                        id: id_a,
                        name: "a".into(),
                    }),
                    2,
                ),
                envelope(
                    Ev::Replace(Entry {
                        id: id_a,
                        name: "a2".into(),
                    }),
                    3,
                ),
                envelope(Ev::Drop(id_b), 4),
            ])
            .await
            .expect("project");
        assert_eq!(
            repo.calls(),
            vec![
                "upsert:a".to_string(),
                "upsert:a2".to_string(),
                format!("delete:{id_b}"),
            ],
        );
    }

    #[tokio::test]
    async fn repository_error_propagates_and_halts_batch() {
        let repo = Arc::new(RecordingRepo::default());
        *repo.fail_next.lock().expect("lock") = true;
        let projector = CatalogProjector::new("p", repo.clone(), classify);
        let result = projector
            .project(&[
                envelope(
                    Ev::Add(Entry {
                        id: Uuid::new_v4(),
                        name: "a".into(),
                    }),
                    1,
                ),
                envelope(
                    Ev::Add(Entry {
                        id: Uuid::new_v4(),
                        name: "b".into(),
                    }),
                    2,
                ),
            ])
            .await;
        assert!(result.is_err());
        assert!(repo.calls().is_empty(), "second upsert must not happen");
    }

    #[tokio::test]
    async fn name_is_static_label_provided_at_construction() {
        let repo = Arc::new(RecordingRepo::default());
        let projector = CatalogProjector::new("vector_index_projector", repo, classify);
        assert_eq!(projector.name(), "vector_index_projector");
    }
}
