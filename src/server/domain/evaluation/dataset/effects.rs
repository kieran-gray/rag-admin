use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateQuestionEffect {
    pub dataset_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureMapBuildEffect {
    pub dataset_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollMapStatusEffect {
    pub dataset_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyMapReadyEffect {
    pub map_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationDatasetEffect {
    EnsureMapBuild(EnsureMapBuildEffect),
    PollMapStatus(PollMapStatusEffect),
    AttemptQuestionGeneration(GenerateQuestionEffect),
    NotifyMapReady(NotifyMapReadyEffect),
}
