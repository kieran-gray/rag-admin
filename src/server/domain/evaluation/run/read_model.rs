use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;
use crate::shared::{
    ChunkingConfig, ChunkingVariant, EvaluationAutotuneRequest, EvaluationMetrics,
    EvaluationResultSplit, EvaluationRunOptions,
};

use super::{
    aggregate::EvaluationRunStatus, events::RetrievalTraceEntry, scoring_policy::ScoringPolicy,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationVariantResultDto {
    pub run_id: Uuid,
    pub variant_label: String,
    pub variant_config: ChunkingConfig,
    pub options: EvaluationRunOptions,
    pub split: EvaluationResultSplit,
    pub recall_mean: f32,
    pub recall_std: f32,
    pub precision_mean: f32,
    pub precision_std: f32,
    pub iou_mean: f32,
    pub iou_std: f32,
    pub precision_omega_mean: f32,
    pub precision_omega_std: f32,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub selected: bool,
    pub retrieval_traces: Vec<RetrievalTraceEntry>,
}

impl EvaluationVariantResultDto {
    pub fn metrics(&self) -> EvaluationMetrics {
        EvaluationMetrics {
            recall_mean: self.recall_mean,
            recall_std: self.recall_std,
            precision_mean: self.precision_mean,
            precision_std: self.precision_std,
            iou_mean: self.iou_mean,
            iou_std: self.iou_std,
            precision_omega_mean: self.precision_omega_mean,
            precision_omega_std: self.precision_omega_std,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunReadModel {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub status: EvaluationRunStatus,
    pub variants_count: u32,
    pub variants_prepared: u32,
    pub variants_scored: u32,
    pub failure_reason: Option<String>,
    pub scoring_policy: ScoringPolicy,
    pub created_at: Timestamp,
    pub variant_results: Vec<EvaluationVariantResultDto>,
}

#[derive(Debug, Clone)]
pub struct NewRunSummary {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub variants_count: u32,
    pub scoring_policy: ScoringPolicy,
    pub created_at: Timestamp,
}

impl From<EvaluationVariantResultDto> for crate::shared::EvaluationVariantResult {
    fn from(v: EvaluationVariantResultDto) -> Self {
        Self {
            variant: ChunkingVariant {
                label: v.variant_label,
                config: v.variant_config,
            },
            options: v.options,
            split: v.split,
            selected: v.selected,
            metrics: EvaluationMetrics {
                recall_mean: v.recall_mean,
                recall_std: v.recall_std,
                precision_mean: v.precision_mean,
                precision_std: v.precision_std,
                iou_mean: v.iou_mean,
                iou_std: v.iou_std,
                precision_omega_mean: v.precision_omega_mean,
                precision_omega_std: v.precision_omega_std,
            },
            chunk_count: 0,
            average_chunk_tokens: 0,
            average_retrieved_tokens: 0,
            question_results: Vec::new(),
        }
    }
}
