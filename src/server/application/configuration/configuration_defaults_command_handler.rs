use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::configuration::defaults::{
    ConfigurationDefaults, ConfigurationDefaultsCommand, SetDefaultChunkingConfiguration,
    SetDefaultPipelineConfiguration, SetDefaultSweepTemplate,
};
use event_sourcing::CommandProcessor;

pub struct ConfigurationDefaultsCommandHandler {
    processor: Arc<CommandProcessor<ConfigurationDefaults>>,
}

impl ConfigurationDefaultsCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<ConfigurationDefaults>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn set_default_chunking_configuration(&self, id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            ConfigurationDefaultsCommand::SetDefaultChunkingConfiguration(
                SetDefaultChunkingConfiguration {
                    chunking_configuration_id: id,
                },
            ),
        )
        .await
    }

    pub async fn set_default_pipeline_configuration(&self, id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            ConfigurationDefaultsCommand::SetDefaultPipelineConfiguration(
                SetDefaultPipelineConfiguration {
                    pipeline_configuration_id: id,
                },
            ),
        )
        .await
    }

    pub async fn set_default_sweep_template(&self, id: Uuid) -> Result<(), AppError> {
        self.dispatch(ConfigurationDefaultsCommand::SetDefaultSweepTemplate(
            SetDefaultSweepTemplate {
                sweep_template_id: id,
            },
        ))
        .await
    }

    async fn dispatch(&self, command: ConfigurationDefaultsCommand) -> Result<(), AppError> {
        self.processor
            .handle(ConfigurationDefaults::singleton_id(), command)
            .await?;
        Ok(())
    }
}
