use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{server::event_sourcing::Aggregate, shared::ChunkingConfig};

use super::{
    commands::IndexingCommand,
    events::{
        ChunkingCompleted, ChunkingRequeued, EmbeddingCompleted, EmbeddingRequeued,
        IndexingCompleted, IndexingEvent, IndexingRemoved, IndexingRequeued, IngestRequested,
        IngestionFailed, IngestionRetried,
    },
    exceptions::IndexingError,
    status::{IndexingStatus, IngestStage},
};

const INDEXING_NAMESPACE: Uuid = uuid::uuid!("e3b0a3d2-f1c4-4b8a-9e7d-2a6c5f891034");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Indexing {
    pub indexing_id: Uuid,
    pub document_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub chunk_set_id: Option<Uuid>,
    pub embedding_set_id: Option<Uuid>,
    pub status: IndexingStatus,
    pub attempts: u32,
    pub last_request_id: Option<Uuid>,
    pub removed: bool,
    pub auto_advance: bool,
}

impl Indexing {
    pub fn compute_id(document_id: Uuid, pipeline_configuration_id: Uuid) -> Uuid {
        let name = format!("{document_id}:{pipeline_configuration_id}");
        Uuid::new_v5(&INDEXING_NAMESPACE, name.as_bytes())
    }

    fn from_requested(e: &IngestRequested) -> Self {
        Self {
            indexing_id: Self::compute_id(e.document_id, e.pipeline_configuration_id),
            document_id: e.document_id,
            pipeline_configuration_id: e.pipeline_configuration_id,
            document_version: e.document_version,
            chunking_config: e.chunking_config,
            chunk_set_id: None,
            embedding_set_id: None,
            status: IndexingStatus::Pending,
            attempts: 1,
            last_request_id: Some(e.request_id),
            removed: false,
            auto_advance: e.auto_advance,
        }
    }
}

impl Aggregate for Indexing {
    type Event = IndexingEvent;
    type Command = IndexingCommand;
    type Error = IndexingError;

    fn aggregate_type() -> &'static str {
        "indexing"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::IngestRequested(e) => {
                self.document_version = e.document_version;
                self.chunking_config = e.chunking_config;
                self.chunk_set_id = None;
                self.embedding_set_id = None;
                self.status = IndexingStatus::Pending;
                self.attempts += 1;
                self.last_request_id = Some(e.request_id);
                self.auto_advance = e.auto_advance;
            }
            Self::Event::ChunkingCompleted(e) => {
                self.chunk_set_id = Some(e.chunk_set_id);
                self.status = IndexingStatus::Chunking;
            }
            Self::Event::EmbeddingCompleted(e) => {
                self.embedding_set_id = Some(e.embedding_set_id);
                self.status = IndexingStatus::Embedding;
            }
            Self::Event::IndexingCompleted(_) => {
                self.status = IndexingStatus::Indexed;
            }
            Self::Event::IngestionFailed(e) => {
                self.status = IndexingStatus::Failed {
                    stage: e.stage.clone(),
                };
            }
            Self::Event::IngestionRetried(e) => {
                self.last_request_id = Some(e.request_id);
                self.status = match &self.status {
                    IndexingStatus::Failed { stage } => match stage {
                        IngestStage::Fetching | IngestStage::Chunking => IndexingStatus::Pending,
                        IngestStage::Embedding => IndexingStatus::Chunking,
                        IngestStage::Indexing => IndexingStatus::Embedding,
                    },
                    other => other.clone(),
                };
            }
            Self::Event::IndexingRemoved(_) => {
                self.removed = true;
            }
            // Markers — purely for policy dispatch, no state change.
            Self::Event::ChunkingRequeued(_)
            | Self::Event::EmbeddingRequeued(_)
            | Self::Event::IndexingRequeued(_) => {}
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::RequestIngest(cmd) => match state {
                None => Ok(vec![Self::Event::IngestRequested(IngestRequested {
                    document_id: cmd.document_id,
                    pipeline_configuration_id: cmd.pipeline_configuration_id,
                    document_version: cmd.document_version,
                    chunking_config: cmd.chunking_config,
                    request_id: cmd.request_id,
                    auto_advance: cmd.auto_advance,
                    occurred_at: cmd.occurred_at,
                })]),
                Some(s) if s.removed => Err(IndexingError::Removed),
                Some(s) if s.last_request_id == Some(cmd.request_id) => Ok(vec![]),
                Some(_) => Ok(vec![Self::Event::IngestRequested(IngestRequested {
                    document_id: cmd.document_id,
                    pipeline_configuration_id: cmd.pipeline_configuration_id,
                    document_version: cmd.document_version,
                    chunking_config: cmd.chunking_config,
                    request_id: cmd.request_id,
                    auto_advance: cmd.auto_advance,
                    occurred_at: cmd.occurred_at,
                })]),
            },

            Self::Command::CompleteChunking(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if s.status.is_at_least_chunking() {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::ChunkingCompleted(ChunkingCompleted {
                    chunk_set_id: cmd.chunk_set_id,
                    chunk_count: cmd.chunk_count,
                    auto_advance: s.auto_advance,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::CompleteEmbedding(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if s.status.is_at_least_embedding() {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::EmbeddingCompleted(EmbeddingCompleted {
                    embedding_set_id: cmd.embedding_set_id,
                    auto_advance: s.auto_advance,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::CompleteIndexing(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if s.status.is_indexed() {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::IndexingCompleted(IndexingCompleted {
                    vector_count: cmd.vector_count,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::FailIngestion(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                Ok(vec![Self::Event::IngestionFailed(IngestionFailed {
                    stage: cmd.stage,
                    reason: cmd.reason,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RetryIngestion(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if !s.status.is_failed() {
                    return Err(IndexingError::NotFailed);
                }
                Ok(vec![Self::Event::IngestionRetried(IngestionRetried {
                    request_id: cmd.request_id,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RemoveIndexing(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::IndexingRemoved(IndexingRemoved {
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RequeueChunking(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                Ok(vec![Self::Event::ChunkingRequeued(ChunkingRequeued {
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RequeueEmbedding(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if s.chunk_set_id.is_none() {
                    return Err(IndexingError::ValidationError(
                        "cannot requeue embedding before chunking has completed".into(),
                    ));
                }
                Ok(vec![Self::Event::EmbeddingRequeued(EmbeddingRequeued {
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RequeueIndexing(cmd) => {
                let s = state.ok_or(IndexingError::NotFound)?;
                if s.removed {
                    return Err(IndexingError::Removed);
                }
                if s.embedding_set_id.is_none() {
                    return Err(IndexingError::ValidationError(
                        "cannot requeue indexing before embedding has completed".into(),
                    ));
                }
                Ok(vec![Self::Event::IndexingRequeued(IndexingRequeued {
                    occurred_at: cmd.occurred_at,
                })])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;

        for event in events {
            match (&mut state, event) {
                (None, Self::Event::IngestRequested(e)) => {
                    state = Some(Self::from_requested(e));
                }
                (None, _) => return None,
                (Some(indexing), event) => indexing.apply(event),
            }
        }

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::indexing::commands::*;
    use crate::server::domain::indexing::events::*;
    use crate::server::domain::shared::Timestamp;
    use crate::shared::SectionChunkingConfig;

    fn now() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn base_request(doc_id: Uuid, pc_id: Uuid) -> IndexingCommand {
        IndexingCommand::RequestIngest(RequestIngest {
            document_id: doc_id,
            pipeline_configuration_id: pc_id,
            document_version: 1,
            chunking_config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: 512,
            }),
            request_id: Uuid::new_v4(),
            auto_advance: true,
            occurred_at: now(),
        })
    }

    fn base_state(doc_id: Uuid, pc_id: Uuid) -> Indexing {
        let events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        Indexing::from_events(&events).unwrap()
    }

    #[test]
    fn request_ingest_from_none_creates_aggregate() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], IndexingEvent::IngestRequested(_)));

        let indexing = Indexing::from_events(&events).unwrap();
        assert_eq!(indexing.document_id, doc_id);
        assert_eq!(indexing.status, IndexingStatus::Pending);
        assert_eq!(indexing.attempts, 1);
        assert_eq!(indexing.indexing_id, Indexing::compute_id(doc_id, pc_id));
    }

    #[test]
    fn duplicate_request_id_is_noop() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let first_events = Indexing::handle_command(
            None,
            IndexingCommand::RequestIngest(RequestIngest {
                document_id: doc_id,
                pipeline_configuration_id: pc_id,
                document_version: 1,
                chunking_config: ChunkingConfig::Section(SectionChunkingConfig {
                    max_section_tokens: 512,
                }),
                request_id,
                auto_advance: true,
                occurred_at: now(),
            }),
        )
        .unwrap();
        let indexing = Indexing::from_events(&first_events).unwrap();

        let second = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RequestIngest(RequestIngest {
                document_id: doc_id,
                pipeline_configuration_id: pc_id,
                document_version: 1,
                chunking_config: ChunkingConfig::Section(SectionChunkingConfig {
                    max_section_tokens: 512,
                }),
                request_id,
                auto_advance: true,
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert!(second.is_empty());
    }

    #[test]
    fn requeue_chunking_emits_marker_event() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let indexing = base_state(doc_id, pc_id);

        let events = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RequeueChunking(RequeueChunking { occurred_at: now() }),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], IndexingEvent::ChunkingRequeued(_)));
    }

    #[test]
    fn requeue_embedding_requires_chunk_set() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let indexing = base_state(doc_id, pc_id);

        let err = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RequeueEmbedding(RequeueEmbedding { occurred_at: now() }),
        )
        .unwrap_err();
        assert!(matches!(err, IndexingError::ValidationError(_)));
    }

    #[test]
    fn requeue_embedding_succeeds_after_chunking() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 10,
            auto_advance: true,
            occurred_at: now(),
        }));
        let indexing = Indexing::from_events(&events).unwrap();

        let out = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RequeueEmbedding(RequeueEmbedding { occurred_at: now() }),
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], IndexingEvent::EmbeddingRequeued(_)));
    }

    #[test]
    fn requeue_indexing_requires_embedding_set() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let indexing = base_state(doc_id, pc_id);

        let err = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RequeueIndexing(RequeueIndexing { occurred_at: now() }),
        )
        .unwrap_err();
        assert!(matches!(err, IndexingError::ValidationError(_)));
    }

    #[test]
    fn requeue_events_do_not_change_status_or_ids() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let chunk_set_id = Uuid::new_v4();
        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id,
            chunk_count: 5,
            auto_advance: true,
            occurred_at: now(),
        }));
        events.push(IndexingEvent::ChunkingRequeued(ChunkingRequeued {
            occurred_at: now(),
        }));

        let indexing = Indexing::from_events(&events).unwrap();
        assert_eq!(indexing.status, IndexingStatus::Chunking);
        assert_eq!(indexing.chunk_set_id, Some(chunk_set_id));
    }

    #[test]
    fn auto_advance_carried_through_event() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let events = Indexing::handle_command(
            None,
            IndexingCommand::RequestIngest(RequestIngest {
                document_id: doc_id,
                pipeline_configuration_id: pc_id,
                document_version: 1,
                chunking_config: ChunkingConfig::Section(SectionChunkingConfig {
                    max_section_tokens: 512,
                }),
                request_id: Uuid::new_v4(),
                auto_advance: false,
                occurred_at: now(),
            }),
        )
        .unwrap();
        let indexing = Indexing::from_events(&events).unwrap();
        assert!(!indexing.auto_advance);
    }

    #[test]
    fn complete_chunking_advances_to_chunking() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let indexing = base_state(doc_id, pc_id);
        let chunk_set_id = Uuid::new_v4();

        let events = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::CompleteChunking(CompleteChunking {
                chunk_set_id,
                chunk_count: 42,
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        if let IndexingEvent::ChunkingCompleted(e) = &events[0] {
            assert_eq!(e.chunk_set_id, chunk_set_id);
        } else {
            panic!("expected ChunkingCompleted");
        }

        let mut all = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        all.extend(events);
        let updated = Indexing::from_events(&all).unwrap();
        assert_eq!(updated.status, IndexingStatus::Chunking);
        assert_eq!(updated.chunk_set_id, Some(chunk_set_id));
    }

    #[test]
    fn complete_chunking_is_idempotent() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let chunk_set_id = Uuid::new_v4();

        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id,
            chunk_count: 10,
            auto_advance: true,
            occurred_at: now(),
        }));
        let indexing = Indexing::from_events(&events).unwrap();

        let repeat = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::CompleteChunking(CompleteChunking {
                chunk_set_id: Uuid::new_v4(),
                chunk_count: 10,
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert!(repeat.is_empty());
    }

    #[test]
    fn full_pipeline_sequence_ends_at_indexed() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();

        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 10,
            auto_advance: true,
            occurred_at: now(),
        }));
        events.push(IndexingEvent::EmbeddingCompleted(EmbeddingCompleted {
            embedding_set_id: Uuid::new_v4(),
            auto_advance: true,
            occurred_at: now(),
        }));
        events.push(IndexingEvent::IndexingCompleted(IndexingCompleted {
            vector_count: 100,
            occurred_at: now(),
        }));

        let indexing = Indexing::from_events(&events).unwrap();
        assert_eq!(indexing.status, IndexingStatus::Indexed);
        assert!(indexing.chunk_set_id.is_some());
        assert!(indexing.embedding_set_id.is_some());
    }

    #[test]
    fn fail_then_retry_resets_to_previous_stage() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();

        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 10,
            auto_advance: true,
            occurred_at: now(),
        }));
        events.push(IndexingEvent::IngestionFailed(IngestionFailed {
            stage: IngestStage::Embedding,
            reason: "connection refused".to_string(),
            occurred_at: now(),
        }));
        let failed = Indexing::from_events(&events).unwrap();
        assert!(failed.status.is_failed());

        let retry_events = Indexing::handle_command(
            Some(&failed),
            IndexingCommand::RetryIngestion(RetryIngestion {
                request_id: Uuid::new_v4(),
                occurred_at: now(),
            }),
        )
        .unwrap();
        assert_eq!(retry_events.len(), 1);

        let mut all = events;
        all.extend(retry_events);
        let retried = Indexing::from_events(&all).unwrap();
        // Failed at Embedding → reset to Chunking (chunking was done)
        assert_eq!(retried.status, IndexingStatus::Chunking);
    }

    #[test]
    fn retry_when_not_failed_returns_error() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let indexing = base_state(doc_id, pc_id);

        let err = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::RetryIngestion(RetryIngestion {
                request_id: Uuid::new_v4(),
                occurred_at: now(),
            }),
        )
        .unwrap_err();

        assert!(matches!(err, IndexingError::NotFailed));
    }

    #[test]
    fn remove_is_idempotent() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();

        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::IndexingRemoved(IndexingRemoved {
            occurred_at: now(),
        }));
        let removed = Indexing::from_events(&events).unwrap();

        let second = Indexing::handle_command(
            Some(&removed),
            IndexingCommand::RemoveIndexing(RemoveIndexing { occurred_at: now() }),
        )
        .unwrap();

        assert!(second.is_empty());
    }

    #[test]
    fn two_different_pipeline_ids_produce_different_aggregate_ids() {
        let doc_id = Uuid::new_v4();
        let pc1 = Uuid::new_v4();
        let pc2 = Uuid::new_v4();

        assert_ne!(
            Indexing::compute_id(doc_id, pc1),
            Indexing::compute_id(doc_id, pc2)
        );
    }

    #[test]
    fn same_inputs_produce_same_aggregate_id() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        assert_eq!(
            Indexing::compute_id(doc_id, pc_id),
            Indexing::compute_id(doc_id, pc_id)
        );
    }

    #[test]
    fn replay_requires_ingest_requested_as_first_event() {
        let result =
            Indexing::from_events(&[IndexingEvent::ChunkingCompleted(ChunkingCompleted {
                chunk_set_id: Uuid::new_v4(),
                chunk_count: 5,
                auto_advance: true,
                occurred_at: now(),
            })]);
        assert!(result.is_none());
    }

    #[test]
    fn complete_embedding_is_idempotent_when_already_embedding() {
        let doc_id = Uuid::new_v4();
        let pc_id = Uuid::new_v4();
        let embedding_set_id = Uuid::new_v4();

        let mut events = Indexing::handle_command(None, base_request(doc_id, pc_id)).unwrap();
        events.push(IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 10,
            auto_advance: true,
            occurred_at: now(),
        }));
        events.push(IndexingEvent::EmbeddingCompleted(EmbeddingCompleted {
            embedding_set_id,
            auto_advance: true,
            occurred_at: now(),
        }));
        let indexing = Indexing::from_events(&events).unwrap();

        let repeat = Indexing::handle_command(
            Some(&indexing),
            IndexingCommand::CompleteEmbedding(CompleteEmbedding {
                embedding_set_id: Uuid::new_v4(),
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert!(repeat.is_empty());
    }
}
