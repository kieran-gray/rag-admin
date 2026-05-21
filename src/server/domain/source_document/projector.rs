use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::SourceDocumentEvent;
use super::read_model::SourceDocumentReadModel;
use super::repository::SourceDocumentRepository;

pub struct SourceDocumentProjector {
    repository: Arc<dyn SourceDocumentRepository>,
}

impl SourceDocumentProjector {
    pub const NAME: &'static str = "source_document_projector";

    pub fn new(repository: Arc<dyn SourceDocumentRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<SourceDocumentEvent> for SourceDocumentProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<SourceDocumentEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let document_id = envelope.metadata.stream_id;
            match &envelope.event {
                SourceDocumentEvent::DocumentCreated(_) => {}
                SourceDocumentEvent::VersionAdded(e) => {
                    let existing = self.repository.load(document_id).await?;
                    let read_model = if let Some(mut m) = existing {
                        m.latest_version_number = e.version_number;
                        m.latest_content_hash = e.content_hash.clone();
                        m.latest_metadata = e.metadata.clone();
                        m.latest_version_occurred_at = e.occurred_at.to_string();
                        m
                    } else {
                        let created = events
                            .iter()
                            .filter(|env| {
                                env.metadata.stream_id == document_id
                                    && env.metadata.log_position < envelope.metadata.log_position
                            })
                            .filter_map(|env| match &env.event {
                                SourceDocumentEvent::DocumentCreated(c) => Some(c),
                                _ => None,
                            })
                            .next_back()
                            .ok_or_else(|| {
                                ProjectionError::Storage(format!(
                                    "VersionAdded for {document_id} without prior DocumentCreated"
                                ))
                            })?;
                        SourceDocumentReadModel {
                            document_id,
                            document_type: created.document_type.clone(),
                            source_ref: created.source_ref.clone(),
                            latest_version_number: e.version_number,
                            latest_content_hash: e.content_hash.clone(),
                            latest_metadata: e.metadata.clone(),
                            latest_version_occurred_at: e.occurred_at.to_string(),
                            deleted: false,
                        }
                    };
                    self.repository.save(read_model).await?;
                }
                SourceDocumentEvent::DocumentDeleted(_) => {
                    if let Some(mut m) = self.repository.load(document_id).await? {
                        m.deleted = true;
                        self.repository.save(m).await?;
                    }
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

    use crate::server::domain::shared::Timestamp;
    use crate::server::domain::source_document::document_type::DocumentType;
    use crate::server::domain::source_document::events::{
        DocumentCreated, DocumentDeleted, VersionAdded,
    };
    use crate::server::domain::source_document::repository::{
        DocumentListPage, DocumentListQuery, SourceDocumentRepositoryError, SourceFilter,
    };
    use crate::server::domain::source_document::source_ref::SourceRef;
    use crate::server::domain::source_document::version::{
        ContentHash, DocumentMetadata, PlainMetadata,
    };

    #[derive(Default)]
    struct InMemDocRepo {
        records: Mutex<HashMap<Uuid, SourceDocumentReadModel>>,
        fail_next_load: Mutex<bool>,
    }

    impl InMemDocRepo {
        fn get(&self, id: Uuid) -> Option<SourceDocumentReadModel> {
            self.records.lock().expect("lock").get(&id).cloned()
        }
    }

    #[async_trait]
    impl SourceDocumentRepository for InMemDocRepo {
        async fn load(
            &self,
            document_id: Uuid,
        ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
            if *self.fail_next_load.lock().expect("lock") {
                *self.fail_next_load.lock().expect("lock") = false;
                return Err(SourceDocumentRepositoryError::Internal("forced".into()));
            }
            Ok(self
                .records
                .lock()
                .expect("lock")
                .get(&document_id)
                .cloned())
        }

        async fn save(
            &self,
            read_model: SourceDocumentReadModel,
        ) -> Result<(), SourceDocumentRepositoryError> {
            self.records
                .lock()
                .expect("lock")
                .insert(read_model.document_id, read_model);
            Ok(())
        }

        async fn list(
            &self,
        ) -> Result<Vec<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
            Ok(self
                .records
                .lock()
                .expect("lock")
                .values()
                .cloned()
                .collect())
        }

        async fn list_page(
            &self,
            _query: &DocumentListQuery,
        ) -> Result<DocumentListPage, SourceDocumentRepositoryError> {
            Ok(DocumentListPage {
                items: Vec::new(),
                next_cursor: None,
                total_matching: 0,
                total_all: 0,
            })
        }

        async fn source_facets(
            &self,
        ) -> Result<Vec<(SourceFilter, u64)>, SourceDocumentRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_source_ref(
            &self,
            _source_ref: &SourceRef,
        ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
            Ok(None)
        }

        async fn connectors_by_documents(
            &self,
            _document_ids: &[Uuid],
        ) -> Result<Vec<(Uuid, Uuid)>, SourceDocumentRepositoryError> {
            Ok(Vec::new())
        }

        async fn document_ids_for_connectors(
            &self,
            _connector_ids: &[Uuid],
        ) -> Result<Vec<Uuid>, SourceDocumentRepositoryError> {
            Ok(Vec::new())
        }

        async fn connector_facets(
            &self,
        ) -> Result<Vec<(Uuid, u64)>, SourceDocumentRepositoryError> {
            Ok(Vec::new())
        }
    }

    fn plain_metadata(title: &str) -> DocumentMetadata {
        DocumentMetadata::Markdown(PlainMetadata {
            title: title.into(),
        })
    }

    fn ts(value: &str) -> Timestamp {
        value.into()
    }

    fn created_envelope(
        document_id: Uuid,
        log_position: i64,
    ) -> EventEnvelope<SourceDocumentEvent> {
        EventEnvelope {
            event: SourceDocumentEvent::DocumentCreated(DocumentCreated {
                document_id,
                document_type: DocumentType::Markdown,
                source_ref: SourceRef::Upload {
                    upload_id: Uuid::nil(),
                },
                occurred_at: ts("2024-01-01T00:00:00Z"),
            }),
            metadata: EventMetadata {
                stream_id: document_id,
                aggregate_type: "source_document".into(),
                sequence: log_position,
                log_position,
                event_type: "DocumentCreated".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn version_envelope(
        document_id: Uuid,
        version_number: u32,
        log_position: i64,
        title: &str,
        content_hash: &str,
        when: &str,
    ) -> EventEnvelope<SourceDocumentEvent> {
        EventEnvelope {
            event: SourceDocumentEvent::VersionAdded(VersionAdded {
                version_number,
                content_hash: ContentHash::new(content_hash.into()),
                metadata: plain_metadata(title),
                occurred_at: ts(when),
            }),
            metadata: EventMetadata {
                stream_id: document_id,
                aggregate_type: "source_document".into(),
                sequence: log_position,
                log_position,
                event_type: "VersionAdded".into(),
                occurred_at: when.into(),
            },
        }
    }

    fn deleted_envelope(
        document_id: Uuid,
        log_position: i64,
    ) -> EventEnvelope<SourceDocumentEvent> {
        EventEnvelope {
            event: SourceDocumentEvent::DocumentDeleted(DocumentDeleted {
                occurred_at: ts("2024-02-01T00:00:00Z"),
            }),
            metadata: EventMetadata {
                stream_id: document_id,
                aggregate_type: "source_document".into(),
                sequence: log_position,
                log_position,
                event_type: "DocumentDeleted".into(),
                occurred_at: "2024-02-01T00:00:00Z".into(),
            },
        }
    }

    #[tokio::test]
    async fn document_created_alone_does_not_create_read_model() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[created_envelope(id, 1)])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn document_created_then_version_added_bootstraps_read_model() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "hash1", "2024-01-01T00:00:00Z"),
            ])
            .await
            .expect("project");

        let model = repo.get(id).expect("present");
        assert_eq!(model.document_id, id);
        assert_eq!(model.latest_version_number, 1);
        assert_eq!(model.latest_content_hash.as_hex(), "hash1");
        assert!(matches!(model.document_type, DocumentType::Markdown));
        assert!(!model.deleted);
    }

    #[tokio::test]
    async fn version_added_without_prior_created_returns_error() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        let result = projector
            .project(&[version_envelope(
                id,
                1,
                1,
                "v1",
                "h",
                "2024-01-01T00:00:00Z",
            )])
            .await;
        assert!(result.is_err());
        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn version_added_with_existing_read_model_updates_latest_fields() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "hash1", "2024-01-01T00:00:00Z"),
            ])
            .await
            .expect("seed");

        projector
            .project(&[version_envelope(
                id,
                2,
                3,
                "v2",
                "hash2",
                "2024-02-01T00:00:00Z",
            )])
            .await
            .expect("project");

        let model = repo.get(id).unwrap();
        assert_eq!(model.latest_version_number, 2);
        assert_eq!(model.latest_content_hash.as_hex(), "hash2");
        assert_eq!(model.latest_version_occurred_at, "2024-02-01T00:00:00Z");
        let title = match &model.latest_metadata {
            DocumentMetadata::Markdown(m) => &m.title,
            _ => panic!("expected markdown"),
        };
        assert_eq!(title, "v2");
    }

    #[tokio::test]
    async fn two_versions_in_same_batch_apply_in_order() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
                version_envelope(id, 2, 3, "v2", "h2", "2024-02-01T00:00:00Z"),
            ])
            .await
            .expect("project");

        let model = repo.get(id).unwrap();
        assert_eq!(model.latest_version_number, 2);
        assert_eq!(model.latest_content_hash.as_hex(), "h2");
    }

    #[tokio::test]
    async fn document_deleted_marks_existing_read_model() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
                deleted_envelope(id, 3),
            ])
            .await
            .expect("project");

        assert!(repo.get(id).unwrap().deleted);
    }

    #[tokio::test]
    async fn document_deleted_for_missing_record_is_silent_no_op() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[deleted_envelope(id, 1)])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn version_added_after_deletion_updates_latest_but_keeps_deleted_flag() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
                deleted_envelope(id, 3),
                version_envelope(id, 2, 4, "v2", "h2", "2024-02-01T00:00:00Z"),
            ])
            .await
            .expect("project");

        let model = repo.get(id).unwrap();
        assert!(
            model.deleted,
            "deletion flag must persist across later VersionAdded"
        );
        assert_eq!(model.latest_version_number, 2);
        assert_eq!(model.latest_content_hash.as_hex(), "h2");
    }

    #[tokio::test]
    async fn multi_stream_batch_isolates_per_stream() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(a, 1),
                created_envelope(b, 2),
                version_envelope(a, 1, 3, "a-v1", "ha", "2024-01-01T00:00:00Z"),
                version_envelope(b, 1, 4, "b-v1", "hb", "2024-01-01T00:00:00Z"),
                deleted_envelope(a, 5),
            ])
            .await
            .expect("project");

        let ma = repo.get(a).unwrap();
        let mb = repo.get(b).unwrap();
        assert!(ma.deleted);
        assert!(!mb.deleted);
        assert_eq!(ma.latest_content_hash.as_hex(), "ha");
        assert_eq!(mb.latest_content_hash.as_hex(), "hb");
    }

    #[tokio::test]
    async fn batch_passes_filter_only_for_lower_log_positions() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        let envs = vec![
            version_envelope(id, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
            created_envelope(id, 3),
        ];
        let result = projector.project(&envs).await;
        assert!(
            result.is_err(),
            "VersionAdded must not pair with a DocumentCreated at a higher log_position",
        );
    }

    #[tokio::test]
    async fn unrelated_created_in_batch_does_not_satisfy_version_added() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let target = Uuid::new_v4();
        let other = Uuid::new_v4();

        let result = projector
            .project(&[
                created_envelope(other, 1),
                version_envelope(target, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
            ])
            .await;
        assert!(result.is_err());
        assert!(repo.get(target).is_none());
    }

    #[tokio::test]
    async fn repository_load_error_propagates() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                created_envelope(id, 1),
                version_envelope(id, 1, 2, "v1", "h1", "2024-01-01T00:00:00Z"),
            ])
            .await
            .expect("seed");

        *repo.fail_next_load.lock().expect("lock") = true;
        let result = projector
            .project(&[version_envelope(
                id,
                2,
                3,
                "v2",
                "h2",
                "2024-02-01T00:00:00Z",
            )])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn projector_name_is_stable_constant() {
        let repo = Arc::new(InMemDocRepo::default());
        let projector = SourceDocumentProjector::new(repo);
        assert_eq!(projector.name(), SourceDocumentProjector::NAME);
        assert_eq!(SourceDocumentProjector::NAME, "source_document_projector");
    }
}
