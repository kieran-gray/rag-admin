use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::{
    AddChunkingConfiguration, ChunkingConfigurationCatalog, ChunkingConfigurationCatalogCommand,
    ChunkingConfigurationCatalogEvent, RemoveChunkingConfiguration, UpdateChunkingConfiguration,
};
use crate::shared::contracts::ChunkingConfigurationCommandDto;
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::CommandProcessor;

pub struct ChunkingConfigurationCatalogCommandHandler {
    processor: Arc<CommandProcessor<ChunkingConfigurationCatalog>>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
}

impl ChunkingConfigurationCatalogCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<ChunkingConfigurationCatalog>>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            defaults,
        })
    }

    pub async fn handle_dto(
        &self,
        command: ChunkingConfigurationCommandDto,
    ) -> Result<(), AppError> {
        match command {
            ChunkingConfigurationCommandDto::CreateChunkingConfiguration(d) => {
                let is_default = d.is_default;
                let appended = self
                    .dispatch(
                        ChunkingConfigurationCatalogCommand::AddChunkingConfiguration(
                            AddChunkingConfiguration {
                                name: d.name,
                                config: d.config,
                            },
                        ),
                    )
                    .await?;
                if is_default {
                    if let Some(id) = added_id(&appended) {
                        self.defaults.set_default_chunking_configuration(id).await?;
                    }
                }
                Ok(())
            }
            ChunkingConfigurationCommandDto::UpdateChunkingConfiguration(d) => {
                self.dispatch(
                    ChunkingConfigurationCatalogCommand::UpdateChunkingConfiguration(
                        UpdateChunkingConfiguration {
                            chunking_configuration_id: d.chunking_configuration_id,
                            name: d.name,
                            config: d.config,
                        },
                    ),
                )
                .await?;
                if d.is_default {
                    self.defaults
                        .set_default_chunking_configuration(d.chunking_configuration_id)
                        .await?;
                }
                Ok(())
            }
            ChunkingConfigurationCommandDto::DeleteChunkingConfiguration(d) => self
                .dispatch(
                    ChunkingConfigurationCatalogCommand::RemoveChunkingConfiguration(
                        RemoveChunkingConfiguration {
                            chunking_configuration_id: d.chunking_configuration_id,
                        },
                    ),
                )
                .await
                .map(|_| ()),
        }
    }

    async fn dispatch(
        &self,
        command: ChunkingConfigurationCatalogCommand,
    ) -> Result<Vec<EventEnvelope<ChunkingConfigurationCatalogEvent>>, AppError> {
        let appended = self
            .processor
            .handle(ChunkingConfigurationCatalog::singleton_id(), command)
            .await?;
        Ok(appended)
    }
}

fn added_id(events: &[EventEnvelope<ChunkingConfigurationCatalogEvent>]) -> Option<Uuid> {
    events.iter().find_map(|e| match &e.event {
        ChunkingConfigurationCatalogEvent::ChunkingConfigurationAdded(added) => {
            Some(added.chunking_configuration_id)
        }
        _ => None,
    })
}
