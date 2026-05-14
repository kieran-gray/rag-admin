use uuid::Uuid;

use crate::server::domain::shared::Timestamp;
use crate::shared::{
    ChunkingConfig, ChunkingVariant, EvaluationAutotuneRequest, EvaluationMetrics,
    EvaluationResultSplit, EvaluationRunOptions,
};

use super::{events::RetrievalTraceEntry, scoring_policy::ScoringPolicy};

pub struct RequestRun {
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

pub enum EvaluationRunCommand {
    RequestRun(RequestRun),
    MarkVariantPrepared(MarkVariantPrepared),
    ScoreVariant(ScoreVariant),
    CompleteRun(CompleteRun),
    FailRun(FailRun),
}

impl EvaluationRunCommand {
    pub fn run_id(&self) -> Uuid {
        match self {
            Self::RequestRun(c) => c.run_id,
            Self::MarkVariantPrepared(c) => c.run_id,
            Self::ScoreVariant(c) => c.run_id,
            Self::CompleteRun(c) => c.run_id,
            Self::FailRun(c) => c.run_id,
        }
    }
}
