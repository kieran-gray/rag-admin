use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::GenerationModel;
use super::events::GenerationModelCatalogEvent;

pub fn make_generation_model_projector(
    repository: Arc<dyn CatalogRepository<GenerationModel>>,
) -> CatalogProjector<GenerationModelCatalogEvent, GenerationModel> {
    CatalogProjector::new("generation_model_projector", repository, classify)
}

fn classify(event: &GenerationModelCatalogEvent) -> CatalogProjectorAction<GenerationModel> {
    match event {
        GenerationModelCatalogEvent::GenerationModelCatalogCreated(_) => {
            CatalogProjectorAction::Noop
        }
        GenerationModelCatalogEvent::GenerationModelAdded(e) => {
            CatalogProjectorAction::Upsert(GenerationModel {
                generation_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
            })
        }
        GenerationModelCatalogEvent::GenerationModelUpdated(e) => {
            CatalogProjectorAction::Upsert(GenerationModel {
                generation_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
            })
        }
        GenerationModelCatalogEvent::GenerationModelRemoved(e) => {
            CatalogProjectorAction::Remove(e.model_id)
        }
    }
}
