use std::sync::Arc;

use crate::server::domain::configuration::catalog::{
    CatalogProjector, CatalogProjectorAction, CatalogRepository,
};

use super::entity::RetrievalProfile;
use super::events::RetrievalProfileCatalogEvent;

pub fn make_retrieval_profile_projector(
    repository: Arc<dyn CatalogRepository<RetrievalProfile>>,
) -> CatalogProjector<RetrievalProfileCatalogEvent, RetrievalProfile> {
    CatalogProjector::new("retrieval_profile_projector", repository, classify)
}

fn classify(event: &RetrievalProfileCatalogEvent) -> CatalogProjectorAction<RetrievalProfile> {
    match event {
        RetrievalProfileCatalogEvent::RetrievalProfileCatalogCreated(_) => {
            CatalogProjectorAction::Noop
        }
        RetrievalProfileCatalogEvent::RetrievalProfileAdded(e) => {
            CatalogProjectorAction::Upsert(RetrievalProfile {
                retrieval_profile_id: e.retrieval_profile_id,
                name: e.name.clone(),
                index_profile_id: e.index_profile_id,
                generation_model_id: e.generation_model_id,
                reranker_model_id: e.reranker_model_id,
                default_top_k: e.default_top_k,
                default_min_score_milli: e.default_min_score_milli,
            })
        }
        RetrievalProfileCatalogEvent::RetrievalProfileUpdated(e) => {
            CatalogProjectorAction::Upsert(RetrievalProfile {
                retrieval_profile_id: e.retrieval_profile_id,
                name: e.name.clone(),
                index_profile_id: e.index_profile_id,
                generation_model_id: e.generation_model_id,
                reranker_model_id: e.reranker_model_id,
                default_top_k: e.default_top_k,
                default_min_score_milli: e.default_min_score_milli,
            })
        }
        RetrievalProfileCatalogEvent::RetrievalProfileRemoved(e) => {
            CatalogProjectorAction::Remove(e.retrieval_profile_id)
        }
    }
}
