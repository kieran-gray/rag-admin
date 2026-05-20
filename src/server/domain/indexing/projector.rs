use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::IndexingEvent;
use super::read_model::IndexingReadModel;
use super::repository::IndexingRepository;
use super::status::{IndexingStatus, IngestStage};

pub struct IndexingProjector {
    repository: Arc<dyn IndexingRepository>,
}

impl IndexingProjector {
    pub const NAME: &'static str = "indexing_projector";

    pub fn new(repository: Arc<dyn IndexingRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<IndexingEvent> for IndexingProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<IndexingEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let indexing_id = envelope.metadata.stream_id;
            match &envelope.event {
                IndexingEvent::IngestRequested(e) => {
                    let read_model = match self.repository.load(indexing_id).await? {
                        Some(mut m) => {
                            m.document_version = e.document_version;
                            m.chunking_config = e.chunking_config.into();
                            m.chunk_set_id = None;
                            m.embedding_set_id = None;
                            m.status = IndexingStatus::Pending;
                            m.attempts += 1;
                            m.auto_advance = e.auto_advance;
                            m
                        }
                        None => IndexingReadModel {
                            indexing_id,
                            document_id: e.document_id,
                            pipeline_configuration_id: e.pipeline_configuration_id,
                            document_version: e.document_version,
                            chunking_config: e.chunking_config.into(),
                            chunk_set_id: None,
                            embedding_set_id: None,
                            status: IndexingStatus::Pending,
                            attempts: 1,
                            removed: false,
                            auto_advance: e.auto_advance,
                        },
                    };
                    self.repository.save(read_model).await?;
                }
                IndexingEvent::ChunkingCompleted(e) => {
                    self.update(indexing_id, |m| {
                        m.chunk_set_id = Some(e.chunk_set_id);
                        m.status = IndexingStatus::Chunking;
                    })
                    .await?;
                }
                IndexingEvent::EmbeddingCompleted(e) => {
                    self.update(indexing_id, |m| {
                        m.embedding_set_id = Some(e.embedding_set_id);
                        m.status = IndexingStatus::Embedding;
                    })
                    .await?;
                }
                IndexingEvent::IndexingCompleted(_) => {
                    self.update(indexing_id, |m| m.status = IndexingStatus::Indexed)
                        .await?;
                }
                IndexingEvent::IngestionFailed(e) => {
                    let stage = e.stage.clone();
                    self.update(indexing_id, |m| {
                        m.status = IndexingStatus::Failed {
                            stage: stage.clone(),
                        };
                    })
                    .await?;
                }
                IndexingEvent::IngestionRetried(_) => {
                    self.update(indexing_id, |m| {
                        m.status = match &m.status {
                            IndexingStatus::Failed { stage } => match stage {
                                IngestStage::Fetching | IngestStage::Chunking => {
                                    IndexingStatus::Pending
                                }
                                IngestStage::Embedding => IndexingStatus::Chunking,
                                IngestStage::Indexing => IndexingStatus::Embedding,
                            },
                            other => other.clone(),
                        };
                    })
                    .await?;
                }
                IndexingEvent::IndexingRemoved(_) => {
                    self.update(indexing_id, |m| m.removed = true).await?;
                }
                IndexingEvent::ChunkingRequeued(_)
                | IndexingEvent::EmbeddingRequeued(_)
                | IndexingEvent::IndexingRequeued(_) => {}
            }
        }
        Ok(())
    }
}

impl IndexingProjector {
    async fn update<F>(&self, indexing_id: uuid::Uuid, mutate: F) -> Result<(), ProjectionError>
    where
        F: FnOnce(&mut IndexingReadModel),
    {
        if let Some(mut m) = self.repository.load(indexing_id).await? {
            mutate(&mut m);
            self.repository.save(m).await?;
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

    use crate::server::domain::indexing::events::{
        ChunkingCompleted, ChunkingRequeued, EmbeddingCompleted, EmbeddingRequeued,
        IndexingCompleted, IndexingRemoved, IndexingRequeued, IngestRequested, IngestionFailed,
        IngestionRetried,
    };
    use crate::server::domain::indexing::repository::IndexingRepositoryError;
    use crate::server::domain::shared::event_payloads::{
        ChunkingConfigPayload, SectionChunkingConfigPayload,
    };

    #[derive(Default)]
    struct InMemIndexingRepo {
        records: Mutex<HashMap<Uuid, IndexingReadModel>>,
        fail_next_load: Mutex<bool>,
    }

    impl InMemIndexingRepo {
        fn get(&self, id: Uuid) -> Option<IndexingReadModel> {
            self.records.lock().expect("lock").get(&id).cloned()
        }
    }

    #[async_trait]
    impl IndexingRepository for InMemIndexingRepo {
        async fn load(
            &self,
            indexing_id: Uuid,
        ) -> Result<Option<IndexingReadModel>, IndexingRepositoryError> {
            if *self.fail_next_load.lock().expect("lock") {
                *self.fail_next_load.lock().expect("lock") = false;
                return Err(IndexingRepositoryError::Internal("forced".into()));
            }
            Ok(self
                .records
                .lock()
                .expect("lock")
                .get(&indexing_id)
                .cloned())
        }

        async fn save(&self, read_model: IndexingReadModel) -> Result<(), IndexingRepositoryError> {
            self.records
                .lock()
                .expect("lock")
                .insert(read_model.indexing_id, read_model);
            Ok(())
        }

        async fn list_for_document(
            &self,
            _document_id: Uuid,
        ) -> Result<Vec<IndexingReadModel>, IndexingRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_for_documents(
            &self,
            _document_ids: &[Uuid],
        ) -> Result<Vec<IndexingReadModel>, IndexingRepositoryError> {
            Ok(Vec::new())
        }
    }

    fn chunking_payload() -> ChunkingConfigPayload {
        ChunkingConfigPayload::Section(SectionChunkingConfigPayload {
            max_section_tokens: 512,
        })
    }

    fn ingest_requested(
        indexing_id: Uuid,
        document_id: Uuid,
        pipeline_id: Uuid,
        version: u32,
        auto_advance: bool,
    ) -> EventEnvelope<IndexingEvent> {
        EventEnvelope {
            event: IndexingEvent::IngestRequested(IngestRequested {
                document_id,
                pipeline_configuration_id: pipeline_id,
                document_version: version,
                chunking_config: chunking_payload(),
                request_id: Uuid::new_v4(),
                auto_advance,
                occurred_at: "2024-01-01T00:00:00Z".into(),
            }),
            metadata: EventMetadata {
                stream_id: indexing_id,
                aggregate_type: "indexing".into(),
                sequence: 1,
                log_position: 1,
                event_type: "IngestRequested".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn envelope(
        indexing_id: Uuid,
        event: IndexingEvent,
        log_position: i64,
    ) -> EventEnvelope<IndexingEvent> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id: indexing_id,
                aggregate_type: "indexing".into(),
                sequence: log_position,
                log_position,
                event_type: "Ev".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    #[tokio::test]
    async fn ingest_requested_initialises_read_model_with_pending_status() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();
        let pipeline_id = Uuid::new_v4();

        projector
            .project(&[ingest_requested(
                indexing_id,
                document_id,
                pipeline_id,
                3,
                true,
            )])
            .await
            .expect("project");

        let model = repo.get(indexing_id).expect("read model");
        assert_eq!(model.indexing_id, indexing_id);
        assert_eq!(model.document_id, document_id);
        assert_eq!(model.pipeline_configuration_id, pipeline_id);
        assert_eq!(model.document_version, 3);
        assert_eq!(model.status, IndexingStatus::Pending);
        assert!(model.chunk_set_id.is_none());
        assert!(model.embedding_set_id.is_none());
        assert_eq!(model.attempts, 1);
        assert!(!model.removed);
        assert!(model.auto_advance);
    }

    #[tokio::test]
    async fn second_ingest_requested_resets_progress_and_increments_attempts() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();
        let pipeline_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, document_id, pipeline_id, 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id: Uuid::new_v4(),
                        chunk_count: 4,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                ingest_requested(indexing_id, document_id, pipeline_id, 2, false),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).expect("read model");
        assert_eq!(model.document_version, 2);
        assert_eq!(model.attempts, 2);
        assert!(model.chunk_set_id.is_none());
        assert!(model.embedding_set_id.is_none());
        assert_eq!(model.status, IndexingStatus::Pending);
        assert!(!model.auto_advance);
    }

    #[tokio::test]
    async fn chunking_completed_records_chunk_set_id_and_advances_status() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();
        let chunk_set_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id,
                        chunk_count: 7,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).unwrap();
        assert_eq!(model.chunk_set_id, Some(chunk_set_id));
        assert_eq!(model.status, IndexingStatus::Chunking);
    }

    #[tokio::test]
    async fn embedding_then_indexing_completed_walk_status_forward() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();
        let embedding_set_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id: Uuid::new_v4(),
                        chunk_count: 5,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::EmbeddingCompleted(EmbeddingCompleted {
                        embedding_set_id,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IndexingCompleted(IndexingCompleted {
                        vector_count: 99,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    4,
                ),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).unwrap();
        assert_eq!(model.embedding_set_id, Some(embedding_set_id));
        assert_eq!(model.status, IndexingStatus::Indexed);
    }

    #[tokio::test]
    async fn ingestion_failed_records_stage_in_status() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionFailed(IngestionFailed {
                        stage: IngestStage::Embedding,
                        reason: "boom".into(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).unwrap();
        assert_eq!(
            model.status,
            IndexingStatus::Failed {
                stage: IngestStage::Embedding,
            },
        );
    }

    #[tokio::test]
    async fn retry_after_embedding_failure_rolls_back_to_chunking_status() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id: Uuid::new_v4(),
                        chunk_count: 5,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionFailed(IngestionFailed {
                        stage: IngestStage::Embedding,
                        reason: "boom".into(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionRetried(IngestionRetried {
                        request_id: Uuid::new_v4(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    4,
                ),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).unwrap();
        assert_eq!(model.status, IndexingStatus::Chunking);
    }

    #[tokio::test]
    async fn retry_after_indexing_failure_rolls_back_to_embedding_status() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionFailed(IngestionFailed {
                        stage: IngestStage::Indexing,
                        reason: "x".into(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionRetried(IngestionRetried {
                        request_id: Uuid::new_v4(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
            ])
            .await
            .expect("project");

        assert_eq!(
            repo.get(indexing_id).unwrap().status,
            IndexingStatus::Embedding,
        );
    }

    #[tokio::test]
    async fn retry_after_chunking_failure_returns_to_pending() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionFailed(IngestionFailed {
                        stage: IngestStage::Chunking,
                        reason: "x".into(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionRetried(IngestionRetried {
                        request_id: Uuid::new_v4(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
            ])
            .await
            .expect("project");

        assert_eq!(
            repo.get(indexing_id).unwrap().status,
            IndexingStatus::Pending
        );
    }

    #[tokio::test]
    async fn retry_after_fetching_failure_returns_to_pending() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionFailed(IngestionFailed {
                        stage: IngestStage::Fetching,
                        reason: "x".into(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionRetried(IngestionRetried {
                        request_id: Uuid::new_v4(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
            ])
            .await
            .expect("project");

        assert_eq!(
            repo.get(indexing_id).unwrap().status,
            IndexingStatus::Pending
        );
    }

    #[tokio::test]
    async fn retry_when_not_failed_leaves_status_unchanged() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id: Uuid::new_v4(),
                        chunk_count: 5,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IngestionRetried(IngestionRetried {
                        request_id: Uuid::new_v4(),
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
            ])
            .await
            .expect("project");

        assert_eq!(
            repo.get(indexing_id).unwrap().status,
            IndexingStatus::Chunking
        );
    }

    #[tokio::test]
    async fn indexing_removed_marks_record_as_removed() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::IndexingRemoved(IndexingRemoved {
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        assert!(repo.get(indexing_id).unwrap().removed);
    }

    #[tokio::test]
    async fn requeue_events_are_no_ops_on_read_model() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                ingest_requested(indexing_id, Uuid::new_v4(), Uuid::new_v4(), 1, true),
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingRequeued(ChunkingRequeued {
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::EmbeddingRequeued(EmbeddingRequeued {
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    3,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IndexingRequeued(IndexingRequeued {
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    4,
                ),
            ])
            .await
            .expect("project");

        let model = repo.get(indexing_id).unwrap();
        assert_eq!(model.status, IndexingStatus::Pending);
        assert_eq!(model.attempts, 1);
    }

    #[tokio::test]
    async fn stage_events_for_missing_record_are_silent_no_ops() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[
                envelope(
                    indexing_id,
                    IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                        chunk_set_id: Uuid::new_v4(),
                        chunk_count: 1,
                        auto_advance: true,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    1,
                ),
                envelope(
                    indexing_id,
                    IndexingEvent::IndexingCompleted(IndexingCompleted {
                        vector_count: 0,
                        occurred_at: "2024-01-01T00:00:00Z".into(),
                    }),
                    2,
                ),
            ])
            .await
            .expect("project");

        assert!(repo.get(indexing_id).is_none());
    }

    #[tokio::test]
    async fn repository_load_error_propagates_through_project() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo.clone());
        let indexing_id = Uuid::new_v4();

        projector
            .project(&[ingest_requested(
                indexing_id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                1,
                true,
            )])
            .await
            .expect("seed");

        *repo.fail_next_load.lock().expect("lock") = true;
        let result = projector
            .project(&[envelope(
                indexing_id,
                IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                    chunk_set_id: Uuid::new_v4(),
                    chunk_count: 1,
                    auto_advance: true,
                    occurred_at: "2024-01-01T00:00:00Z".into(),
                }),
                2,
            )])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn projector_name_is_stable_constant() {
        let repo = Arc::new(InMemIndexingRepo::default());
        let projector = IndexingProjector::new(repo);
        assert_eq!(projector.name(), IndexingProjector::NAME);
        assert_eq!(IndexingProjector::NAME, "indexing_projector");
    }
}
