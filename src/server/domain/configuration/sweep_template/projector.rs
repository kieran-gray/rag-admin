use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

use super::events::SweepTemplateEvent;
use super::read_model::SweepTemplateReadModel;
use super::repository::SweepTemplateRepository;

pub struct SweepTemplateProjector {
    repository: Arc<dyn SweepTemplateRepository>,
}

impl SweepTemplateProjector {
    pub const NAME: &'static str = "sweep_template_projector";

    pub fn new(repository: Arc<dyn SweepTemplateRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<SweepTemplateEvent> for SweepTemplateProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(&self, events: &[EventEnvelope<SweepTemplateEvent>]) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                SweepTemplateEvent::SweepTemplateCreated(e) => {
                    self.repository
                        .save(SweepTemplateReadModel {
                            sweep_template_id: e.sweep_template_id,
                            name: e.name.clone(),
                            members: e.members.clone(),
                            is_default: false,
                        })
                        .await?;
                }
                SweepTemplateEvent::SweepTemplateUpdated(e) => {
                    self.repository
                        .save(SweepTemplateReadModel {
                            sweep_template_id: e.sweep_template_id,
                            name: e.name.clone(),
                            members: e.members.clone(),
                            is_default: false,
                        })
                        .await?;
                }
                SweepTemplateEvent::SweepTemplateDeleted(e) => {
                    self.repository.delete(e.sweep_template_id).await?;
                }
                SweepTemplateEvent::SweepTemplateDefaultSet(e) => {
                    self.repository.set_default(e.sweep_template_id).await?;
                }
            }
        }
        Ok(())
    }
}
