use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::IndexProfile;
use super::events::IndexProfileCatalogEvent;

pub fn make_index_profile_projector(
    repository: Arc<dyn CatalogRepository<IndexProfile>>,
) -> CatalogProjector<IndexProfileCatalogEvent, IndexProfile> {
    CatalogProjector::new("index_profile_projector", repository, classify)
}

fn classify(event: &IndexProfileCatalogEvent) -> CatalogProjectorAction<IndexProfile> {
    match event {
        IndexProfileCatalogEvent::IndexProfileCatalogCreated(_) => CatalogProjectorAction::Noop,
        IndexProfileCatalogEvent::IndexProfileAdded(e) => {
            CatalogProjectorAction::Upsert(IndexProfile {
                index_profile_id: e.index_profile_id,
                name: e.name.clone(),
                embedding_model_id: e.embedding_model_id,
                vector_index_id: e.vector_index_id,
                dimensions: e.dimensions,
            })
        }
        IndexProfileCatalogEvent::IndexProfileUpdated(e) => {
            CatalogProjectorAction::Upsert(IndexProfile {
                index_profile_id: e.index_profile_id,
                name: e.name.clone(),
                embedding_model_id: e.embedding_model_id,
                vector_index_id: e.vector_index_id,
                dimensions: e.dimensions,
            })
        }
        IndexProfileCatalogEvent::IndexProfileRemoved(e) => {
            CatalogProjectorAction::Remove(e.index_profile_id)
        }
    }
}
