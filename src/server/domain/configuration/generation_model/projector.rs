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

#[cfg(test)]
mod tests {
    use super::super::events::{
        GenerationModelAdded, GenerationModelCatalogCreated, GenerationModelRemoved,
        GenerationModelUpdated,
    };
    use super::*;
    use crate::shared::reference_data::AiProviderKind;
    use uuid::Uuid;

    #[test]
    fn catalog_created_classifies_as_noop() {
        let ev = GenerationModelCatalogEvent::GenerationModelCatalogCreated(
            GenerationModelCatalogCreated {
                catalog_id: Uuid::nil(),
            },
        );
        assert!(matches!(classify(&ev), CatalogProjectorAction::Noop));
    }

    #[test]
    fn added_event_yields_upsert_with_full_entry() {
        let id = Uuid::new_v4();
        let ev = GenerationModelCatalogEvent::GenerationModelAdded(GenerationModelAdded {
            model_id: id,
            kind: AiProviderKind::Ollama,
            model: "llama3".into(),
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.generation_model_id, id);
        assert_eq!(entry.kind, AiProviderKind::Ollama);
        assert_eq!(entry.model, "llama3");
    }

    #[test]
    fn updated_event_yields_upsert_preserving_id() {
        let id = Uuid::new_v4();
        let ev = GenerationModelCatalogEvent::GenerationModelUpdated(GenerationModelUpdated {
            model_id: id,
            kind: AiProviderKind::Cloudflare,
            model: "@cf/meta/llama-3".into(),
        });
        let CatalogProjectorAction::Upsert(entry) = classify(&ev) else {
            panic!("expected upsert");
        };
        assert_eq!(entry.generation_model_id, id);
        assert_eq!(entry.kind, AiProviderKind::Cloudflare);
    }

    #[test]
    fn removed_event_yields_remove_with_same_id() {
        let id = Uuid::new_v4();
        let ev = GenerationModelCatalogEvent::GenerationModelRemoved(GenerationModelRemoved {
            model_id: id,
        });
        let CatalogProjectorAction::Remove(carried) = classify(&ev) else {
            panic!("expected remove");
        };
        assert_eq!(carried, id);
    }
}
