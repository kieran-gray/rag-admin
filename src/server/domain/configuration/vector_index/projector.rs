use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::VectorIndex;
use super::events::VectorIndexCatalogEvent;

pub fn make_vector_index_projector(
    repository: Arc<dyn CatalogRepository<VectorIndex>>,
) -> CatalogProjector<VectorIndexCatalogEvent, VectorIndex> {
    CatalogProjector::new("vector_index_projector", repository, classify)
}

fn classify(event: &VectorIndexCatalogEvent) -> CatalogProjectorAction<VectorIndex> {
    match event {
        VectorIndexCatalogEvent::VectorIndexCatalogCreated(_) => CatalogProjectorAction::Noop,
        VectorIndexCatalogEvent::VectorIndexAdded(e) => {
            CatalogProjectorAction::Upsert(VectorIndex {
                index_id: e.index_id,
                kind: e.kind,
                name: e.name.clone(),
                dimensions: e.dimensions,
            })
        }
        VectorIndexCatalogEvent::VectorIndexUpdated(e) => {
            CatalogProjectorAction::Upsert(VectorIndex {
                index_id: e.index_id,
                kind: e.kind,
                name: e.name.clone(),
                dimensions: e.dimensions,
            })
        }
        VectorIndexCatalogEvent::VectorIndexRemoved(e) => {
            CatalogProjectorAction::Remove(e.index_id)
        }
    }
}
