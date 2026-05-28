use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::insight::Insight;
use super::super::observation::Observation;
use super::super::role::SuggestedRole;
use super::super::thread::Thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMapReadModel {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub section_size: u32,
    pub status: String,
    pub generation_model_id: Uuid,
    pub suggested_roles: Vec<SuggestedRole>,
    pub observations_extracted: u32,
    pub threads_synthesized: u32,
    pub insights_synthesized: u32,
}

#[derive(Debug, Clone)]
pub struct DocumentMapDetail {
    pub read_model: DocumentMapReadModel,
    pub observations: Vec<Observation>,
    pub threads: Vec<Thread>,
    pub insights: Vec<Insight>,
    pub carried_summary: String,
}
