use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::PipelineConfiguration;
use super::events::PipelineConfigurationCatalogEvent;

pub fn make_pipeline_configuration_projector(
    repository: Arc<dyn CatalogRepository<PipelineConfiguration>>,
) -> CatalogProjector<PipelineConfigurationCatalogEvent, PipelineConfiguration> {
    CatalogProjector::new("pipeline_configuration_projector", repository, classify)
}

fn classify(
    event: &PipelineConfigurationCatalogEvent,
) -> CatalogProjectorAction<PipelineConfiguration> {
    match event {
        PipelineConfigurationCatalogEvent::PipelineConfigurationCatalogCreated(_) => {
            CatalogProjectorAction::Noop
        }
        PipelineConfigurationCatalogEvent::PipelineConfigurationAdded(e) => {
            CatalogProjectorAction::Upsert(PipelineConfiguration {
                pipeline_configuration_id: e.pipeline_configuration_id,
                name: e.name.clone(),
                embedding_model_id: e.embedding_model_id,
                generation_model_id: e.generation_model_id,
                vector_index_id: e.vector_index_id,
                dimensions: e.dimensions,
            })
        }
        PipelineConfigurationCatalogEvent::PipelineConfigurationUpdated(e) => {
            CatalogProjectorAction::Upsert(PipelineConfiguration {
                pipeline_configuration_id: e.pipeline_configuration_id,
                name: e.name.clone(),
                embedding_model_id: e.embedding_model_id,
                generation_model_id: e.generation_model_id,
                vector_index_id: e.vector_index_id,
                dimensions: e.dimensions,
            })
        }
        PipelineConfigurationCatalogEvent::PipelineConfigurationRemoved(e) => {
            CatalogProjectorAction::Remove(e.pipeline_configuration_id)
        }
    }
}
