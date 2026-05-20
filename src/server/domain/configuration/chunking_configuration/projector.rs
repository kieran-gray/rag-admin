use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::ChunkingConfiguration;
use super::events::ChunkingConfigurationCatalogEvent;

pub fn make_chunking_configuration_projector(
    repository: Arc<dyn CatalogRepository<ChunkingConfiguration>>,
) -> CatalogProjector<ChunkingConfigurationCatalogEvent, ChunkingConfiguration> {
    CatalogProjector::new("chunking_configuration_projector", repository, classify)
}

fn classify(
    event: &ChunkingConfigurationCatalogEvent,
) -> CatalogProjectorAction<ChunkingConfiguration> {
    match event {
        ChunkingConfigurationCatalogEvent::ChunkingConfigurationCatalogCreated(_) => {
            CatalogProjectorAction::Noop
        }
        ChunkingConfigurationCatalogEvent::ChunkingConfigurationAdded(e) => {
            CatalogProjectorAction::Upsert(ChunkingConfiguration {
                chunking_configuration_id: e.chunking_configuration_id,
                name: e.name.clone(),
                config: e.config,
            })
        }
        ChunkingConfigurationCatalogEvent::ChunkingConfigurationUpdated(e) => {
            CatalogProjectorAction::Upsert(ChunkingConfiguration {
                chunking_configuration_id: e.chunking_configuration_id,
                name: e.name.clone(),
                config: e.config,
            })
        }
        ChunkingConfigurationCatalogEvent::ChunkingConfigurationRemoved(e) => {
            CatalogProjectorAction::Remove(e.chunking_configuration_id)
        }
    }
}
