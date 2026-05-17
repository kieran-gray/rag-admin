use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contracts::QueryHit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub pipeline_configuration_id: Uuid,
    pub query: String,
    pub top_k: u32,
    pub min_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub pipeline_configuration_id: Uuid,
    pub query: String,
    pub answer: String,
    pub model: String,
    pub hits: Vec<QueryHit>,
}
