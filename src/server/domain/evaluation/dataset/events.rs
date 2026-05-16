use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::super::question::EvaluationReference;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetGenerationRequested {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub label: String,
    pub target_question_count: u32,
    pub generation_model_id: Uuid,
    pub generation_model: String,
    pub excerpt_similarity_threshold_milli: u32,
    pub duplicate_similarity_threshold_milli: u32,
    pub embedding_model_id: Uuid,
    #[serde(default)]
    pub max_attempts: u32,
    #[serde(default)]
    pub grammar_variants_enabled: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionAccepted {
    pub dataset_id: Uuid,
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReference>,
    pub embedding: Option<Vec<f32>>,
    pub category: crate::server::domain::evaluation::question::QuestionCategory,
    pub grammar_variant: crate::server::domain::evaluation::question::GrammarVariant,
    pub paraphrase_of: Option<u32>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionRejected {
    pub dataset_id: Uuid,
    pub attempt: u32,
    pub reason: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetGenerationCompleted {
    pub dataset_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetGenerationFailed {
    pub dataset_id: Uuid,
    pub reason: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetRenamed {
    pub dataset_id: Uuid,
    pub label: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetDeleted {
    pub dataset_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationDatasetEvent {
    DatasetGenerationRequested(DatasetGenerationRequested),
    QuestionAccepted(QuestionAccepted),
    QuestionRejected(QuestionRejected),
    DatasetGenerationCompleted(DatasetGenerationCompleted),
    DatasetGenerationFailed(DatasetGenerationFailed),
    DatasetRenamed(DatasetRenamed),
    DatasetDeleted(DatasetDeleted),
}
