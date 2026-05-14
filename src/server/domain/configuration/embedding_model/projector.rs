use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

use super::entity::EmbeddingModel;
use super::events::EmbeddingModelCatalogEvent;
use super::repository::EmbeddingModelRepository;

pub struct EmbeddingModelProjector {
    repository: Arc<dyn EmbeddingModelRepository>,
}

impl EmbeddingModelProjector {
    pub const NAME: &'static str = "embedding_model_projector";

    pub fn new(repository: Arc<dyn EmbeddingModelRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<EmbeddingModelCatalogEvent> for EmbeddingModelProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<EmbeddingModelCatalogEvent>],
    ) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                EmbeddingModelCatalogEvent::EmbeddingModelCatalogCreated(_) => {}
                EmbeddingModelCatalogEvent::EmbeddingModelAdded(e) => {
                    self.repository
                        .save(EmbeddingModel {
                            embedding_model_id: e.model_id,
                            kind: e.kind,
                            model: e.model.clone(),
                            dimensions: e.dimensions,
                        })
                        .await?;
                }
                EmbeddingModelCatalogEvent::EmbeddingModelUpdated(e) => {
                    self.repository
                        .save(EmbeddingModel {
                            embedding_model_id: e.model_id,
                            kind: e.kind,
                            model: e.model.clone(),
                            dimensions: e.dimensions,
                        })
                        .await?;
                }
                EmbeddingModelCatalogEvent::EmbeddingModelRemoved(e) => {
                    self.repository.delete(e.model_id).await?;
                }
            }
        }
        Ok(())
    }
}
