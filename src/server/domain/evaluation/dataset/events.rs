use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::shared::Timestamp;

use super::super::question::{EvaluationReference, QuestionDimensions};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxisWeight {
    pub operation: String,
    pub evidence: String,
    pub weight: u32,
}

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
    pub duplicate_similarity_threshold_milli: u32,
    pub embedding_model_id: Uuid,
    pub max_attempts: u32,
    pub weight_matrix: Vec<AxisWeight>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapDependencyResolved {
    pub dataset_id: Uuid,
    pub map_id: Uuid,
    pub ready: bool,
    pub failure_reason: Option<String>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionAccepted {
    pub dataset_id: Uuid,
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReference>,
    pub embedding: Option<Vec<f32>>,
    pub dimensions: QuestionDimensions,
    pub evidence_refs: Vec<MapItemRef>,
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
pub struct DatasetGenerationCancelled {
    pub dataset_id: Uuid,
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
    MapDependencyResolved(MapDependencyResolved),
    QuestionAccepted(QuestionAccepted),
    QuestionRejected(QuestionRejected),
    DatasetGenerationCompleted(DatasetGenerationCompleted),
    DatasetGenerationFailed(DatasetGenerationFailed),
    DatasetGenerationCancelled(DatasetGenerationCancelled),
    DatasetRenamed(DatasetRenamed),
    DatasetDeleted(DatasetDeleted),
}
