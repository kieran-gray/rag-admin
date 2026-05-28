use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::super::insight::Insight;
use super::super::observation::Observation;
use super::super::role::SuggestedRole;
use super::super::thread::Thread;

pub const DEFAULT_SECTION_SIZE: u32 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapBuildRequested {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub section_size: u32,
    pub generation_model_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolesSuggested {
    pub roles: Vec<SuggestedRole>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationsExtracted {
    pub chunk_sequence: u32,
    pub observations: Vec<Observation>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadsSynthesized {
    pub section_sequence: u32,
    pub threads: Vec<Thread>,
    pub carried_summary: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightsSynthesized {
    pub insights: Vec<Insight>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum DocumentMapEvent {
    MapBuildRequested(MapBuildRequested),
    RolesSuggested(RolesSuggested),
    ObservationsExtracted(ObservationsExtracted),
    ThreadsSynthesized(ThreadsSynthesized),
    InsightsSynthesized(InsightsSynthesized),
}
