use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::aggregate::DatasetGenerationStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetReadModel {
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
    pub status: DatasetGenerationStatus,
    pub question_count: u32,
    pub rejection_count: u32,
    pub failure_reason: Option<String>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone)]
pub struct NewDatasetSummary {
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
    pub created_at: Timestamp,
}
