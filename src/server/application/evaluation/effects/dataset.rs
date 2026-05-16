use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::evaluation::question::QuestionCategory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateQuestionEffect {
    pub dataset_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateParaphraseEffect {
    pub dataset_id: Uuid,
    pub clean_sequence: u32,
    pub category: QuestionCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationDatasetEffect {
    AttemptQuestionGeneration(GenerateQuestionEffect),
    GenerateParaphrase(GenerateParaphraseEffect),
}
