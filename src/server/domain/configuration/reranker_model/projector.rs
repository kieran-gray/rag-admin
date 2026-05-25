use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::RerankerModel;
use super::events::RerankerModelCatalogEvent;

pub fn make_reranker_model_projector(
    repository: Arc<dyn CatalogRepository<RerankerModel>>,
) -> CatalogProjector<RerankerModelCatalogEvent, RerankerModel> {
    CatalogProjector::new("reranker_model_projector", repository, classify)
}

fn classify(event: &RerankerModelCatalogEvent) -> CatalogProjectorAction<RerankerModel> {
    match event {
        RerankerModelCatalogEvent::RerankerModelCatalogCreated(_) => CatalogProjectorAction::Noop,
        RerankerModelCatalogEvent::RerankerModelAdded(e) => {
            CatalogProjectorAction::Upsert(RerankerModel {
                reranker_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
            })
        }
        RerankerModelCatalogEvent::RerankerModelUpdated(e) => {
            CatalogProjectorAction::Upsert(RerankerModel {
                reranker_model_id: e.model_id,
                kind: e.kind,
                model: e.model.clone(),
            })
        }
        RerankerModelCatalogEvent::RerankerModelRemoved(e) => {
            CatalogProjectorAction::Remove(e.model_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::events::{
        RerankerModelAdded, RerankerModelCatalogCreated, RerankerModelRemoved,
        RerankerModelUpdated,
    };
    use super::*;
    use crate::shared::reference_data::AiProviderKind;
    use uuid::Uuid;

    #[test]
    fn catalog_created_classifies_as_noop() {
        let ev = RerankerModelCatalogEvent::RerankerModelCatalogCreated(
            RerankerModelCatalogCreated {
                catalog_id: Uuid::nil(),
            },
        );
        assert!(matches!(classify(&ev), CatalogProjectorAction::Noop));
    }

    #[test]
    fn added_event_yields_upsert_with_full_entry() {
        let id = Uuid::new_v4();
        let ev = RerankerModelCatalogEvent::RerankerModelAdded(RerankerModelAdded {
            model_id: id,
            kind: AiProviderKind::LlamaServer,
            model: "qwen3-reranker-4b".into(),
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.reranker_model_id, id);
        assert_eq!(entry.kind, AiProviderKind::LlamaServer);
        assert_eq!(entry.model, "qwen3-reranker-4b");
    }

    #[test]
    fn updated_event_yields_upsert_preserving_id() {
        let id = Uuid::new_v4();
        let ev = RerankerModelCatalogEvent::RerankerModelUpdated(RerankerModelUpdated {
            model_id: id,
            kind: AiProviderKind::Cloudflare,
            model: "@cf/baai/bge-reranker-base".into(),
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.reranker_model_id, id);
        assert_eq!(entry.kind, AiProviderKind::Cloudflare);
    }

    #[test]
    fn removed_event_yields_remove_with_same_id() {
        let id = Uuid::new_v4();
        let ev = RerankerModelCatalogEvent::RerankerModelRemoved(RerankerModelRemoved {
            model_id: id,
        });
        let CatalogProjectorAction::Remove(carried) = classify(&ev) else {
            panic!("expected remove");
        };
        assert_eq!(carried, id);
    }
}
