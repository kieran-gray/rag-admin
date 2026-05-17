use std::sync::Arc;

use async_trait::async_trait;

use crate::server::domain::configuration::chunking_configuration::{
    ChunkingConfigurationRepository, ChunkingConfigurationRepositoryError,
};
use crate::server::domain::configuration::pipeline_configuration::{
    PipelineConfigurationRepository, PipelineConfigurationRepositoryError,
};
use crate::server::domain::configuration::sweep_template::{
    SweepTemplateRepository, SweepTemplateRepositoryError,
};
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::error::ProjectionError;
use crate::server::event_sourcing::projector::Projector;

use super::events::ConfigurationDefaultsEvent;

pub struct ConfigurationDefaultsProjector {
    chunking: Arc<dyn ChunkingConfigurationRepository>,
    pipeline: Arc<dyn PipelineConfigurationRepository>,
    sweep_template: Arc<dyn SweepTemplateRepository>,
}

pub fn make_configuration_defaults_projector(
    chunking: Arc<dyn ChunkingConfigurationRepository>,
    pipeline: Arc<dyn PipelineConfigurationRepository>,
    sweep_template: Arc<dyn SweepTemplateRepository>,
) -> ConfigurationDefaultsProjector {
    ConfigurationDefaultsProjector {
        chunking,
        pipeline,
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
                    self.chunking
                        .set_default(e.chunking_configuration_id)
                        .await
                        .map_err(map_chunking_err)?;
                }
                ConfigurationDefaultsEvent::DefaultPipelineConfigurationSet(e) => {
                    self.pipeline
                        .set_default(e.pipeline_configuration_id)
                        .await
                        .map_err(map_pipeline_err)?;
                }
                ConfigurationDefaultsEvent::DefaultSweepTemplateSet(e) => {
                    self.sweep_template
                        .set_default(e.sweep_template_id)
                        .await
                        .map_err(map_sweep_err)?;
                }
            }
        }
        Ok(())
    }
}

fn map_chunking_err(err: ChunkingConfigurationRepositoryError) -> ProjectionError {
    use ChunkingConfigurationRepositoryError::*;
    match err {
        NotFound(id) => ProjectionError::InvalidState(format!(
            "chunking configuration {id} missing when applying default"
        )),
        NameConflict | ReferenceViolation(_) | Internal(_) => {
            ProjectionError::Storage(err.to_string())
        }
    }
}

fn map_pipeline_err(err: PipelineConfigurationRepositoryError) -> ProjectionError {
    use PipelineConfigurationRepositoryError::*;
    match err {
        NotFound(id) => ProjectionError::InvalidState(format!(
            "pipeline configuration {id} missing when applying default"
        )),
        NameConflict | ReferenceViolation(_) | Internal(_) => {
            ProjectionError::Storage(err.to_string())
        }
    }
}

fn map_sweep_err(err: SweepTemplateRepositoryError) -> ProjectionError {
    ProjectionError::Storage(err.to_string())
}
