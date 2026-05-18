use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{
    ChunkingConfig, EvaluationMetrics, EvaluationRunOptions, EvaluationVariantResult, OrderedF32,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationReferenceDto {
    pub content: String,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_start: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_end: u32,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationQuestionDto {
    pub question: String,
    pub references: Vec<EvaluationReferenceDto>,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
    pub category: String,
    pub grammar_variant: String,
    pub paraphrase_of: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationJobInfo {
    pub job_id: String,
    pub stream_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetSummaryDto {
    pub dataset_id: Uuid,
    pub label: String,
    pub question_count: u32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetDto {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub label: String,
    pub status: String,
    pub target_question_count: u32,
    pub question_count: u32,
    pub rejection_count: u32,
    pub generation_model_id: Uuid,
    pub generation_model: String,
    pub embedding_model_id: Uuid,
    pub failure_reason: Option<String>,
    pub questions: Vec<EvaluationQuestionDto>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunSummaryDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub status: String,
    pub variant_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub status: String,
    pub variants: Vec<EvaluationVariantResult>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestVariantDto {
    pub label: String,
    pub config: ChunkingConfig,
    pub options: EvaluationRunOptions,
    pub score: f32,
    pub metrics: EvaluationMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEvaluationRunDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_title: Option<String>,
    pub status: String,
    pub variant_count: u32,
    pub created_at: String,
    pub best: Option<BestVariantDto>,
}
