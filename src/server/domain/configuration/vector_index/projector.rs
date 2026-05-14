use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

use super::entity::VectorIndex;
use super::events::VectorIndexCatalogEvent;
use super::repository::VectorIndexRepository;

pub struct VectorIndexProjector {
    repository: Arc<dyn VectorIndexRepository>,
}

impl VectorIndexProjector {
    pub const NAME: &'static str = "vector_index_projector";

    pub fn new(repository: Arc<dyn VectorIndexRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<VectorIndexCatalogEvent> for VectorIndexProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<VectorIndexCatalogEvent>],
    ) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                VectorIndexCatalogEvent::VectorIndexCatalogCreated(_) => {}
                VectorIndexCatalogEvent::VectorIndexAdded(e) => {
                    self.repository
                        .save(VectorIndex {
                            index_id: e.index_id,
                            kind: e.kind,
                            name: e.name.clone(),
                            dimensions: e.dimensions,
                        })
                        .await?;
                }
                VectorIndexCatalogEvent::VectorIndexUpdated(e) => {
                    self.repository
                        .save(VectorIndex {
                            index_id: e.index_id,
                            kind: e.kind,
                            name: e.name.clone(),
                            dimensions: e.dimensions,
                        })
                        .await?;
                }
                VectorIndexCatalogEvent::VectorIndexRemoved(e) => {
                    self.repository.delete(e.index_id).await?;
                }
            }
        }
        Ok(())
    }
}
