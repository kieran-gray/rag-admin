use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{ChunkingVariant, EvaluationRunOptions, OptimizationConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvaluationRequestDto {
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    #[serde(default)]
    pub retrieval_profile_id: Option<Uuid>,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOptimizationRequestDto {
    pub dataset_id: Uuid,
    pub index_profile_id: Uuid,
    #[serde(default)]
    pub retrieval_profile_id: Option<Uuid>,
    pub optimization: OptimizationConfig,
}
