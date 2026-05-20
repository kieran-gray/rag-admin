use std::sync::Arc;

use async_trait::async_trait;

use crate::server::domain::configuration::sweep_template::{
    SweepTemplateRepository, SweepTemplateRepositoryError,
};
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConfigurationDefaultsEvent;
use super::repository::ConfigurationDefaultsRepository;

pub struct ConfigurationDefaultsProjector {
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
    sweep_template: Arc<dyn SweepTemplateRepository>,
}

pub fn make_configuration_defaults_projector(
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
    sweep_template: Arc<dyn SweepTemplateRepository>,
) -> ConfigurationDefaultsProjector {
    ConfigurationDefaultsProjector {
        defaults,
        sweep_template,
    }
}

#[async_trait]
impl Projector<ConfigurationDefaultsEvent> for ConfigurationDefaultsProjector {
    fn name(&self) -> &str {
        "configuration_defaults_projector"
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ConfigurationDefaultsEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            match &envelope.event {
                ConfigurationDefaultsEvent::DefaultChunkingConfigurationSet(e) => {
                    self.defaults
                        .set_chunking_configuration(e.chunking_configuration_id)
                        .await?;
                }
                ConfigurationDefaultsEvent::DefaultPipelineConfigurationSet(e) => {
                    self.defaults
                        .set_pipeline_configuration(e.pipeline_configuration_id)
                        .await?;
                }
                ConfigurationDefaultsEvent::DefaultSweepTemplateSet(e) => {
                    self.sweep_template
                        .set_default(e.sweep_template_id)
                        .await
                        .map_err(|err| map_sweep_err(&err))?;
                }
            }
        }
        Ok(())
    }
}

fn map_sweep_err(err: &SweepTemplateRepositoryError) -> ProjectionError {
    ProjectionError::Storage(err.to_string())
}
