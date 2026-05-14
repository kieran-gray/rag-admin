use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

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

    async fn project(&self, events: &[EventEnvelope<IndexingEvent>]) -> Result<(), AppError> {
        for envelope in events {
            let indexing_id = envelope.metadata.stream_id;
            match &envelope.event {
                IndexingEvent::IngestRequested(e) => {
                    let read_model = match self.repository.load(indexing_id).await? {
                        Some(mut m) => {
                            m.document_version = e.document_version;
                            m.chunking_config = e.chunking_config;
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
                            chunking_config: e.chunking_config,
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
                | IndexingEvent::IndexingRequeued(_) => {
                    // Marker events — no read-model state change.
                }
            }
        }
        Ok(())
    }
}

impl IndexingProjector {
    async fn update<F>(&self, indexing_id: uuid::Uuid, mutate: F) -> Result<(), AppError>
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
