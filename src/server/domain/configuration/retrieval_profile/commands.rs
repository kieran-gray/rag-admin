use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexProfileRef {
    pub index_profile_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GenerationModelRef {
    pub generation_model_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddRetrievalProfile {
    pub name: String,
    pub index_profile: IndexProfileRef,
    pub generation_model: GenerationModelRef,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateRetrievalProfile {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile: IndexProfileRef,
    pub generation_model: GenerationModelRef,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveRetrievalProfile {
    pub retrieval_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RetrievalProfileCatalogCommand {
    AddRetrievalProfile(AddRetrievalProfile),
    UpdateRetrievalProfile(UpdateRetrievalProfile),
    RemoveRetrievalProfile(RemoveRetrievalProfile),
}
