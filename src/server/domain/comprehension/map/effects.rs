use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestRolesEffect {
    pub map_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractObservationsEffect {
    pub map_id: Uuid,
    pub chunk_sequence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizeThreadsEffect {
    pub map_id: Uuid,
    pub section_sequence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizeInsightsEffect {
    pub map_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DocumentMapEffect {
    SuggestRoles(SuggestRolesEffect),
    ExtractObservations(ExtractObservationsEffect),
    SynthesizeThreads(SynthesizeThreadsEffect),
    SynthesizeInsights(SynthesizeInsightsEffect),
}
