use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub pipeline_configuration_id: Uuid,
    pub query: String,
    pub top_k: u32,
    pub min_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHit {
    pub id: String,
    pub score: f32,
    pub document_id: Option<Uuid>,
    pub source_ref_key: Option<String>,
    pub document_title: Option<String>,
    pub chunk_id: Option<Uuid>,
    pub heading: Option<String>,
    pub snippet: String,
    pub char_start: Option<u32>,
    pub char_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub pipeline_configuration_id: Uuid,
    pub query: String,
    pub hits: Vec<QueryHit>,
}
