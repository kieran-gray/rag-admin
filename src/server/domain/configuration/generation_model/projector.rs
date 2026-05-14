use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

use super::entity::GenerationModel;
use super::events::GenerationModelCatalogEvent;
use super::repository::GenerationModelRepository;

pub struct GenerationModelProjector {
    repository: Arc<dyn GenerationModelRepository>,
}

impl GenerationModelProjector {
    pub const NAME: &'static str = "generation_model_projector";

    pub fn new(repository: Arc<dyn GenerationModelRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<GenerationModelCatalogEvent> for GenerationModelProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<GenerationModelCatalogEvent>],
    ) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                GenerationModelCatalogEvent::GenerationModelCatalogCreated(_) => {}
                GenerationModelCatalogEvent::GenerationModelAdded(e) => {
                    self.repository
                        .save(GenerationModel {
                            generation_model_id: e.model_id,
                            kind: e.kind,
                            model: e.model.clone(),
                        })
                        .await?;
                }
                GenerationModelCatalogEvent::GenerationModelUpdated(e) => {
                    self.repository
                        .save(GenerationModel {
                            generation_model_id: e.model_id,
                            kind: e.kind,
                            model: e.model.clone(),
                        })
                        .await?;
                }
                GenerationModelCatalogEvent::GenerationModelRemoved(e) => {
                    self.repository.delete(e.model_id).await?;
                }
            }
        }
        Ok(())
    }
}
