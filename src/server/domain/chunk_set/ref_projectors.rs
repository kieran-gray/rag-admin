use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use crate::server::domain::evaluation::run::events::EvaluationRunEvent;
use crate::server::domain::indexing::events::IndexingEvent;
use crate::server::domain::indexing::repository::IndexingRepository;

use super::repository::ChunkSetRepository;

pub struct IndexingChunkSetRefProjector {
    indexing_repo: Arc<dyn IndexingRepository>,
    chunk_set_repo: Arc<dyn ChunkSetRepository>,
}

impl IndexingChunkSetRefProjector {
    pub const NAME: &'static str = "indexing_chunk_set_ref_projector";

    pub fn new(
        indexing_repo: Arc<dyn IndexingRepository>,
        chunk_set_repo: Arc<dyn ChunkSetRepository>,
    ) -> Self {
        Self {
            indexing_repo,
            chunk_set_repo,
        }
    }
}

#[async_trait]
impl Projector<IndexingEvent> for IndexingChunkSetRefProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<IndexingEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            match &envelope.event {
                IndexingEvent::ChunkingCompleted(e) => {
                    self.chunk_set_repo
                        .bump_indexing_refs(e.chunk_set_id, 1)
                        .await?;
                }
                IndexingEvent::IndexingRemoved(_) => {
                    let indexing_id = envelope.metadata.stream_id;
                    if let Some(m) = self.indexing_repo.load(indexing_id).await? {
                        if let Some(chunk_set_id) = m.chunk_set_id {
                            self.chunk_set_repo
                                .bump_indexing_refs(chunk_set_id, -1)
                                .await?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub struct EvaluationRunChunkSetRefProjector {
    chunk_set_repo: Arc<dyn ChunkSetRepository>,
}

impl EvaluationRunChunkSetRefProjector {
    pub const NAME: &'static str = "evaluation_run_chunk_set_ref_projector";

    pub fn new(chunk_set_repo: Arc<dyn ChunkSetRepository>) -> Self {
        Self { chunk_set_repo }
    }
}

#[async_trait]
impl Projector<EvaluationRunEvent> for EvaluationRunChunkSetRefProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<EvaluationRunEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            if let EvaluationRunEvent::VariantScored(e) = &envelope.event {
                self.chunk_set_repo
                    .bump_variant_result_refs(e.chunk_set_id, 1)
                    .await?;
            }
        }
        Ok(())
    }
}
