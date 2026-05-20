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

#[cfg(test)]
mod tests {
    use super::super::events::{
        VectorIndexAdded, VectorIndexCatalogCreated, VectorIndexRemoved, VectorIndexUpdated,
    };
    use super::*;
    use crate::shared::reference_data::VectorStoreKind;
    use uuid::Uuid;

    #[test]
    fn catalog_created_is_noop() {
        let ev = VectorIndexCatalogEvent::VectorIndexCatalogCreated(VectorIndexCatalogCreated {
            catalog_id: Uuid::nil(),
        });
        assert!(matches!(classify(&ev), CatalogProjectorAction::Noop));
    }

    #[test]
    fn added_event_classifies_as_upsert_carrying_entry() {
        let id = Uuid::new_v4();
        let ev = VectorIndexCatalogEvent::VectorIndexAdded(VectorIndexAdded {
            index_id: id,
            kind: VectorStoreKind::Postgres,
            name: "docs".into(),
            dimensions: 768,
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.index_id, id);
        assert_eq!(entry.name, "docs");
        assert_eq!(entry.dimensions, 768);
    }

    #[test]
    fn updated_event_classifies_as_upsert() {
        let id = Uuid::new_v4();
        let ev = VectorIndexCatalogEvent::VectorIndexUpdated(VectorIndexUpdated {
            index_id: id,
            kind: VectorStoreKind::CloudflareVectorize,
            name: "fresh".into(),
            dimensions: 1024,
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.name, "fresh");
        assert_eq!(entry.kind, VectorStoreKind::CloudflareVectorize);
    }

    #[test]
    fn removed_event_classifies_as_remove_by_id() {
        let id = Uuid::new_v4();
        let ev = VectorIndexCatalogEvent::VectorIndexRemoved(VectorIndexRemoved { index_id: id });
        let CatalogProjectorAction::Remove(carried) = classify(&ev) else {
            panic!("expected remove");
        };
        assert_eq!(carried, id);
    }
}
