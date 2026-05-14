use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::evaluation::run::scoring_policy::ScoringPolicy;
use crate::shared::{ChunkingVariant, EvaluationAutotuneRequest, EvaluationRunOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRunEffect {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub scoring_policy: ScoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvaluationRunEffect {
    ExecuteRun(ExecuteRunEffect),
}
