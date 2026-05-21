use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::evaluation::run::scoring_policy::ScoringPolicy;
use crate::server::domain::evaluation::value_objects::{
    EvaluationAutotuneRequest, EvaluationRunOptions, OptimizationConfig,
};
use crate::server::domain::shared::value_objects::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteVariantEffect {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variant_label: String,
    pub variant_config: ChunkingConfig,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub scoring_policy: ScoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeRunEffect {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub scoring_policy: ScoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeRunEffect {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub optimization: OptimizationConfig,
    pub scoring_policy: ScoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationRunEffect {
    ExecuteVariant(ExecuteVariantEffect),
    FinalizeRun(FinalizeRunEffect),
    OptimizeRun(OptimizeRunEffect),
}
