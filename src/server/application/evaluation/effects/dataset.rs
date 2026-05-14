use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDatasetEffect {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub target_question_count: u32,
    pub generation_model_id: Uuid,
    pub embedding_model_id: Uuid,
    pub excerpt_similarity_threshold_milli: u32,
    pub duplicate_similarity_threshold_milli: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationDatasetEffect {
    GenerateDataset(GenerateDatasetEffect),
}
