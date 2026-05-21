use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::contracts::QueryHit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub retrieval_profile_id: Uuid,
    pub query: String,
    pub top_k: u32,
    pub min_score: f32,
    #[serde(default)]
    pub metadata_filters: Vec<MetadataFilterDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataFilterDto {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub retrieval_profile_id: Uuid,
    pub query: String,
    pub answer: String,
    pub model: String,
    pub hits: Vec<QueryHit>,
}
