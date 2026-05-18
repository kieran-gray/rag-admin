use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::event_payloads::{
    ChunkingConfigPayload, ChunkingVariantPayload, EvaluationAutotuneRequestPayload,
    EvaluationMetricsPayload, EvaluationResultSplitPayload, EvaluationRunOptionsPayload,
    OptimizationConfigPayload, RunFingerprintPayload,
};
use crate::server::domain::shared::Timestamp;

use super::scoring_policy::ScoringPolicy;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalTraceEntry {
    pub question_sequence: u32,
    pub retrieved_chunk_ids: Vec<Uuid>,
    pub scores: Vec<f32>,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRequested {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariantPayload>,
    pub options: Vec<EvaluationRunOptionsPayload>,
    pub autotune_request: Option<EvaluationAutotuneRequestPayload>,
    pub optimization: Option<OptimizationConfigPayload>,
    pub scoring_policy: ScoringPolicy,
    pub fingerprint: RunFingerprintPayload,
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
    pub variant_config: ChunkingConfigPayload,
    pub options: EvaluationRunOptionsPayload,
    pub split: EvaluationResultSplitPayload,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub metrics: EvaluationMetricsPayload,
    pub retrieval_traces: Vec<RetrievalTraceEntry>,
    pub selected: bool,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrialProposed {
    pub run_id: Uuid,
    pub trial_id: u32,
    pub params: serde_json::Value,
    pub rung: u32,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RungAdvanced {
    pub run_id: Uuid,
    pub rung: u32,
    pub surviving_trials: Vec<u32>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionSelected {
    pub run_id: Uuid,
    pub trial_id: u32,
    pub holdout_metrics: EvaluationMetricsPayload,
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
    TrialProposed(TrialProposed),
    RungAdvanced(RungAdvanced),
    ChampionSelected(ChampionSelected),
    RunCompleted(RunCompleted),
    RunFailed(RunFailed),
}
