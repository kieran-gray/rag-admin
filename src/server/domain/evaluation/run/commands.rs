use uuid::Uuid;

use crate::server::domain::evaluation::value_objects::{
    ChunkingVariant, EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
    OptimizationConfig, RunFingerprint,
};
use crate::server::domain::shared::value_objects::ChunkingConfig;
use crate::server::domain::shared::Timestamp;

use super::{events::RetrievalTraceEntry, scoring_policy::ScoringPolicy};

pub struct RequestRun {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    pub retrieval_profile_id: Option<Uuid>,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub optimization: Option<OptimizationConfig>,
    pub scoring_policy: ScoringPolicy,
    pub fingerprint: RunFingerprint,
    pub occurred_at: Timestamp,
}

pub struct MarkVariantPrepared {
    pub run_id: Uuid,
    pub variant_label: String,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct ScoreVariant {
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

pub struct CompleteRun {
    pub run_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct FailRun {
    pub run_id: Uuid,
    pub reason: String,
    pub occurred_at: Timestamp,
}

pub struct ProposeTrial {
    pub run_id: Uuid,
    pub trial_id: u32,
    pub params: serde_json::Value,
    pub rung: u32,
    pub occurred_at: Timestamp,
}

pub struct AdvanceRung {
    pub run_id: Uuid,
    pub rung: u32,
    pub surviving_trials: Vec<u32>,
    pub occurred_at: Timestamp,
}

pub struct SelectChampion {
    pub run_id: Uuid,
    pub trial_id: u32,
    pub holdout_metrics: EvaluationMetrics,
    pub occurred_at: Timestamp,
}

pub enum EvaluationRunCommand {
    RequestRun(RequestRun),
    MarkVariantPrepared(MarkVariantPrepared),
    ScoreVariant(ScoreVariant),
    ProposeTrial(ProposeTrial),
    AdvanceRung(AdvanceRung),
    SelectChampion(SelectChampion),
    CompleteRun(CompleteRun),
    FailRun(FailRun),
}

impl EvaluationRunCommand {
    pub fn run_id(&self) -> Uuid {
        match self {
            Self::RequestRun(c) => c.run_id,
            Self::MarkVariantPrepared(c) => c.run_id,
            Self::ScoreVariant(c) => c.run_id,
            Self::ProposeTrial(c) => c.run_id,
            Self::AdvanceRung(c) => c.run_id,
            Self::SelectChampion(c) => c.run_id,
            Self::CompleteRun(c) => c.run_id,
            Self::FailRun(c) => c.run_id,
        }
    }
}
