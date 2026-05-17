use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::EmbeddingModel;
use super::events::EmbeddingModelCatalogEvent;

pub fn make_embedding_model_projector(
    repository: Arc<dyn CatalogRepository<EmbeddingModel>>,
) -> CatalogProjector<EmbeddingModelCatalogEvent, EmbeddingModel> {
    CatalogProjector::new("embedding_model_projector", repository, classify)
}

fn classify(event: &EmbeddingModelCatalogEvent) -> CatalogProjectorAction<EmbeddingModel> {
    match event {
        EmbeddingModelCatalogEvent::EmbeddingModelCatalogCreated(_) => CatalogProjectorAction::Noop,
        EmbeddingModelCatalogEvent::EmbeddingModelAdded(e) => {
            CatalogProjectorAction::Upsert(EmbeddingModel {
                embedding_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
                dimensions: e.dimensions,
            })
        }
        EmbeddingModelCatalogEvent::EmbeddingModelUpdated(e) => {
            CatalogProjectorAction::Upsert(EmbeddingModel {
                embedding_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
                dimensions: e.dimensions,
            })
        }
        EmbeddingModelCatalogEvent::EmbeddingModelRemoved(e) => {
            CatalogProjectorAction::Remove(e.model_id)
        }
    }
}
