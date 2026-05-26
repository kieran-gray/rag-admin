use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpanDto {
    pub document_id: Uuid,
    pub char_start: u32,
    pub char_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum MapItemRefDto {
    Observation {
        map_id: Uuid,
        observation_id: Uuid,
    },
    Thread {
        map_id: Uuid,
        thread_id: Uuid,
    },
    Insight {
        map_id: Uuid,
        insight_id: Uuid,
    },
    Connection {
        corpus_map_id: Uuid,
        connection_id: Uuid,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestedRoleDto {
    pub name: String,
    pub focus: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationDto {
    pub observation_id: Uuid,
    pub chunk_sequence: u32,
    pub kind: String,
    pub summary: String,
    pub spans: Vec<SpanDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadDto {
    pub thread_id: Uuid,
    pub section_sequence: u32,
    pub kind: String,
    pub summary: String,
    pub evidence: Vec<MapItemRefDto>,
    pub spans: Vec<SpanDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDto {
    pub insight_id: Uuid,
    pub kind: String,
    pub summary: String,
    pub evidence: Vec<MapItemRefDto>,
    pub spans: Vec<SpanDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMapSummaryDto {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub section_size: u32,
    pub status: String,
    pub failure_reason: Option<String>,
    pub generation_model_id: Uuid,
    pub suggested_roles: Vec<SuggestedRoleDto>,
    pub observations_extracted: u32,
    pub threads_synthesized: u32,
    pub insights_synthesized: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMapDetailDto {
    pub summary: DocumentMapSummaryDto,
    pub observations: Vec<ObservationDto>,
    pub threads: Vec<ThreadDto>,
    pub insights: Vec<InsightDto>,
    pub carried_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMapBuildRequestDto {
    pub document_id: Uuid,
    pub document_version: u32,
    pub generation_model_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMapBuildResponseDto {
    pub map_id: Uuid,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub status: String,
}
