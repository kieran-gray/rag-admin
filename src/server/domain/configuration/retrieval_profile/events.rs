use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RetrievalProfileCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RetrievalProfileAdded {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile_id: Uuid,
    pub generation_model_id: Uuid,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RetrievalProfileUpdated {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile_id: Uuid,
    pub generation_model_id: Uuid,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RetrievalProfileRemoved {
    pub retrieval_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum RetrievalProfileCatalogEvent {
    RetrievalProfileCatalogCreated(RetrievalProfileCatalogCreated),
    RetrievalProfileAdded(RetrievalProfileAdded),
    RetrievalProfileUpdated(RetrievalProfileUpdated),
    RetrievalProfileRemoved(RetrievalProfileRemoved),
}
