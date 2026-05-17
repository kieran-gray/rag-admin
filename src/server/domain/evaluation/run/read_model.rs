use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{
    ChunkingConfig, ChunkingVariant, EvaluationAutotuneRequest, EvaluationMetrics,
    EvaluationQuestionResult, EvaluationResultSplit, EvaluationRunOptions, EvaluationVariantResult,
    OptimizationConfig,
};
use crate::server::domain::shared::Timestamp;

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
    pub chunk_count: u32,
    pub average_chunk_tokens: u32,
    pub average_retrieved_tokens: u32,
    pub selected: bool,
    pub retrieval_traces: Vec<RetrievalTraceEntry>,
    pub recall_ci_low: f32,
    pub recall_ci_high: f32,
    pub precision_ci_low: f32,
    pub precision_ci_high: f32,
    pub iou_ci_low: f32,
    pub iou_ci_high: f32,
    pub precision_omega_ci_low: f32,
    pub precision_omega_ci_high: f32,
    pub composite_ci_low: f32,
    pub composite_ci_high: f32,
    pub judge_score: Option<f32>,
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
            chunk_count: self.chunk_count,
            average_chunk_tokens: self.average_chunk_tokens,
            average_retrieved_tokens: self.average_retrieved_tokens,
            recall_ci_low: self.recall_ci_low,
            recall_ci_high: self.recall_ci_high,
            precision_ci_low: self.precision_ci_low,
            precision_ci_high: self.precision_ci_high,
            iou_ci_low: self.iou_ci_low,
            iou_ci_high: self.iou_ci_high,
            precision_omega_ci_low: self.precision_omega_ci_low,
            precision_omega_ci_high: self.precision_omega_ci_high,
            composite_ci_low: self.composite_ci_low,
            composite_ci_high: self.composite_ci_high,
            judge_score: self.judge_score,
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
    pub optimization: Option<OptimizationConfig>,
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
    pub optimization: Option<OptimizationConfig>,
    pub variants_count: u32,
    pub scoring_policy: ScoringPolicy,
    pub created_at: Timestamp,
}

impl From<EvaluationVariantResultDto> for EvaluationVariantResult {
    fn from(v: EvaluationVariantResultDto) -> Self {
        let question_results: Vec<EvaluationQuestionResult> = v
            .retrieval_traces
            .iter()
            .map(|t| EvaluationQuestionResult {
                question: format!("Q{}", t.question_sequence + 1),
                recall: t.recall,
                precision: t.precision,
                iou: t.iou,
                retrieved_chunk_ids: Vec::new(),
                missed_reference_count: 0,
                reference_results: Vec::new(),
                category: t.category.clone(),
            })
            .collect();
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
                recall_ci_low: v.recall_ci_low,
                recall_ci_high: v.recall_ci_high,
                precision_ci_low: v.precision_ci_low,
                precision_ci_high: v.precision_ci_high,
                iou_ci_low: v.iou_ci_low,
                iou_ci_high: v.iou_ci_high,
                precision_omega_ci_low: v.precision_omega_ci_low,
                precision_omega_ci_high: v.precision_omega_ci_high,
                composite_ci_low: v.composite_ci_low,
                composite_ci_high: v.composite_ci_high,
                chunk_count: v.chunk_count,
                average_chunk_tokens: v.average_chunk_tokens,
                average_retrieved_tokens: v.average_retrieved_tokens,
                judge_score: v.judge_score,
            },
            question_results,
        }
    }
}
