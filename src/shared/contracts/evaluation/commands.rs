use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{
    ChunkingVariant, EvaluationAutotuneRequest, EvaluationRunOptions, OptimizationConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvaluationRequestDto {
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    #[serde(default)]
    pub autotune: Option<EvaluationAutotuneRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOptimizationRequestDto {
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub optimization: OptimizationConfig,
}
