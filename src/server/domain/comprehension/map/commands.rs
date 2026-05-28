use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::super::insight::Insight;
use super::super::observation::Observation;
use super::super::role::SuggestedRole;
use super::super::thread::Thread;

pub struct RequestMapBuild {
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

pub struct RecordSuggestedRoles {
    pub map_id: Uuid,
    pub roles: Vec<SuggestedRole>,
    pub occurred_at: Timestamp,
}

pub struct RecordObservations {
    pub map_id: Uuid,
    pub chunk_sequence: u32,
    pub observations: Vec<Observation>,
    pub occurred_at: Timestamp,
}

pub struct RecordThreads {
    pub map_id: Uuid,
    pub section_sequence: u32,
    pub threads: Vec<Thread>,
    pub carried_summary: String,
    pub occurred_at: Timestamp,
}

pub struct RecordInsights {
    pub map_id: Uuid,
    pub insights: Vec<Insight>,
    pub occurred_at: Timestamp,
}

pub enum DocumentMapCommand {
    RequestMapBuild(RequestMapBuild),
    RecordSuggestedRoles(RecordSuggestedRoles),
    RecordObservations(RecordObservations),
    RecordThreads(RecordThreads),
    RecordInsights(RecordInsights),
}

impl DocumentMapCommand {
    pub fn map_id(&self) -> Uuid {
        match self {
            Self::RequestMapBuild(c) => c.map_id,
            Self::RecordSuggestedRoles(c) => c.map_id,
            Self::RecordObservations(c) => c.map_id,
            Self::RecordThreads(c) => c.map_id,
            Self::RecordInsights(c) => c.map_id,
        }
    }
}
