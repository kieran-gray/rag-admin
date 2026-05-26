use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ChunkSetEvent;
use super::repository::ChunkSetRepository;

pub struct ChunkSetProjector {
    repository: Arc<dyn ChunkSetRepository>,
}

impl ChunkSetProjector {
    pub const NAME: &'static str = "chunk_set_projector";

    pub fn new(repository: Arc<dyn ChunkSetRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<ChunkSetEvent> for ChunkSetProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ChunkSetEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let chunk_set_id = envelope.metadata.stream_id;
            match &envelope.event {
                ChunkSetEvent::ChunkSetCreated(e) => {
                    self.repository.project_created(e).await?;
                }
                ChunkSetEvent::ChunkSetPinned(_) => {
                    self.repository.project_pinned(chunk_set_id, true).await?;
                }
                ChunkSetEvent::ChunkSetUnpinned(_) => {
                    self.repository.project_pinned(chunk_set_id, false).await?;
                }
                ChunkSetEvent::ChunkSetDeleted(_) => {
                    self.repository.project_deleted(chunk_set_id).await?;
                }
            }
        }
        Ok(())
    }
}
