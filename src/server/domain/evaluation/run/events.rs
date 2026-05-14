use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;
use crate::shared::{
    ChunkingConfig, ChunkingVariant, EvaluationAutotuneRequest, EvaluationMetrics,
    EvaluationResultSplit, EvaluationRunOptions,
};

use super::scoring_policy::ScoringPolicy;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalTraceEntry {
    pub question_sequence: u32,
    pub retrieved_chunk_ids: Vec<Uuid>,
    pub scores: Vec<f32>,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRequested {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub scoring_policy: ScoringPolicy,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariantPrepared {
    pub run_id: Uuid,
    pub variant_label: String,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantScored {
    pub run_id: Uuid,
    pub variant_label: String,
    pub variant_config: ChunkingConfig,
    pub options: EvaluationRunOptions,
    pub split: EvaluationResultSplit,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub metrics: EvaluationMetrics,
    pub retrieval_traces: Vec<RetrievalTraceEntry>,
    pub selected: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunCompleted {
    pub run_id: Uuid,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunFailed {
    pub run_id: Uuid,
    pub reason: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationRunEvent {
    RunRequested(RunRequested),
    VariantPrepared(VariantPrepared),
    VariantScored(VariantScored),
    RunCompleted(RunCompleted),
    RunFailed(RunFailed),
}
