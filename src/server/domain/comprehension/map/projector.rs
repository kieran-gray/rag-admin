use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::DocumentMapEvent;
use super::repository::DocumentMapRepository;

pub struct DocumentMapProjector {
    repository: Arc<dyn DocumentMapRepository>,
}

impl DocumentMapProjector {
    pub const NAME: &'static str = "document_map_projector";

    pub fn new(repository: Arc<dyn DocumentMapRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<DocumentMapEvent> for DocumentMapProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<DocumentMapEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let map_id = envelope.metadata.stream_id;
            match &envelope.event {
                DocumentMapEvent::MapBuildRequested(e) => {
                    self.repository.project_requested(e).await?;
                }
                DocumentMapEvent::RolesSuggested(e) => {
                    self.repository.project_roles(map_id, &e.roles).await?;
                }
                DocumentMapEvent::ObservationsExtracted(e) => {
                    self.repository
                        .project_observations(map_id, e.chunk_sequence, &e.observations)
                        .await?;
                }
                DocumentMapEvent::ChunkExtractionFailed(e) => {
                    self.repository
                        .project_chunk_extraction_failure(map_id, e.chunk_sequence)
                        .await?;
                }
                DocumentMapEvent::ThreadsSynthesized(e) => {
                    self.repository
                        .project_threads(map_id, e.section_sequence, &e.carried_summary, &e.threads)
                        .await?;
                }
                DocumentMapEvent::SectionSynthesisFailed(e) => {
                    self.repository
                        .project_section_synthesis_failure(map_id, e.section_sequence)
                        .await?;
                }
                DocumentMapEvent::InsightsSynthesized(e) => {
                    self.repository
                        .project_insights(map_id, &e.insights)
                        .await?;
                }
                DocumentMapEvent::MapFailed(e) => {
                    self.repository.project_failure(map_id, &e.reason).await?;
                }
            }
        }
        Ok(())
    }
}
