use serde::{Deserialize, Serialize};

use crate::shared::{
    ChunkingVariant, EvaluationAutotuneRequest, EvaluationMetrics, EvaluationResultSplit,
    EvaluationRunOptions, OptimizationBudget, OptimizationConfig, OptimizationScope,
};

use super::chunking::ChunkingConfigPayload;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunFingerprintPayload {
    pub document_content_hash: String,
    pub dataset_content_hash: String,
    pub embedding_model_snapshot: serde_json::Value,
    pub generation_model_snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingVariantPayload {
    pub label: String,
    pub config: ChunkingConfigPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluationRunOptionsPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub top_k: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub min_score_milli: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationAutotuneRequestPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub tuning_fraction_milli: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub holdout_top_n: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationBudgetPayload {
    Quick,
    #[default]
    Thorough,
    Exhaustive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationScopePayload {
    Chunking,
    Retrieval,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OptimizationConfigPayload {
    pub budget: OptimizationBudgetPayload,
    pub scope: OptimizationScopePayload,
    pub judges_enabled: bool,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResultSplitPayload {
    #[default]
    Full,
    Tuning,
    Validation,
    Holdout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetricsPayload {
    pub recall_mean: f32,
    pub recall_std: f32,
    pub precision_mean: f32,
    pub precision_std: f32,
    pub iou_mean: f32,
    pub iou_std: f32,
    pub precision_omega_mean: f32,
    pub precision_omega_std: f32,
    pub chunk_count: u32,
    pub average_chunk_tokens: u32,
    pub average_retrieved_tokens: u32,
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

impl From<ChunkingVariant> for ChunkingVariantPayload {
    fn from(v: ChunkingVariant) -> Self {
        Self {
            label: v.label,
            config: v.config.into(),
        }
    }
}

impl From<ChunkingVariantPayload> for ChunkingVariant {
    fn from(v: ChunkingVariantPayload) -> Self {
        Self {
            label: v.label,
            config: v.config.into(),
        }
    }
}

impl From<EvaluationRunOptions> for EvaluationRunOptionsPayload {
    fn from(o: EvaluationRunOptions) -> Self {
        Self {
            top_k: o.top_k,
            min_score_milli: o.min_score_milli,
        }
    }
}

impl From<EvaluationRunOptionsPayload> for EvaluationRunOptions {
    fn from(o: EvaluationRunOptionsPayload) -> Self {
        Self {
            top_k: o.top_k,
            min_score_milli: o.min_score_milli,
        }
    }
}

impl From<EvaluationAutotuneRequest> for EvaluationAutotuneRequestPayload {
    fn from(r: EvaluationAutotuneRequest) -> Self {
        Self {
            tuning_fraction_milli: r.tuning_fraction_milli,
            holdout_top_n: r.holdout_top_n,
        }
    }
}

impl From<EvaluationAutotuneRequestPayload> for EvaluationAutotuneRequest {
    fn from(r: EvaluationAutotuneRequestPayload) -> Self {
        Self {
            tuning_fraction_milli: r.tuning_fraction_milli,
            holdout_top_n: r.holdout_top_n,
        }
    }
}

impl From<OptimizationBudget> for OptimizationBudgetPayload {
    fn from(b: OptimizationBudget) -> Self {
        match b {
            OptimizationBudget::Quick => Self::Quick,
            OptimizationBudget::Thorough => Self::Thorough,
            OptimizationBudget::Exhaustive => Self::Exhaustive,
        }
    }
}

impl From<OptimizationBudgetPayload> for OptimizationBudget {
    fn from(b: OptimizationBudgetPayload) -> Self {
        match b {
            OptimizationBudgetPayload::Quick => Self::Quick,
            OptimizationBudgetPayload::Thorough => Self::Thorough,
            OptimizationBudgetPayload::Exhaustive => Self::Exhaustive,
        }
    }
}

impl From<OptimizationScope> for OptimizationScopePayload {
    fn from(s: OptimizationScope) -> Self {
        match s {
            OptimizationScope::Chunking => Self::Chunking,
            OptimizationScope::Retrieval => Self::Retrieval,
            OptimizationScope::Both => Self::Both,
        }
    }
}

impl From<OptimizationScopePayload> for OptimizationScope {
    fn from(s: OptimizationScopePayload) -> Self {
        match s {
            OptimizationScopePayload::Chunking => Self::Chunking,
            OptimizationScopePayload::Retrieval => Self::Retrieval,
            OptimizationScopePayload::Both => Self::Both,
        }
    }
}

impl From<OptimizationConfig> for OptimizationConfigPayload {
    fn from(c: OptimizationConfig) -> Self {
        Self {
            budget: c.budget.into(),
            scope: c.scope.into(),
            judges_enabled: c.judges_enabled,
            seed: c.seed,
        }
    }
}

impl From<OptimizationConfigPayload> for OptimizationConfig {
    fn from(c: OptimizationConfigPayload) -> Self {
        Self {
            budget: c.budget.into(),
            scope: c.scope.into(),
            judges_enabled: c.judges_enabled,
            seed: c.seed,
        }
    }
}

impl From<EvaluationResultSplit> for EvaluationResultSplitPayload {
    fn from(s: EvaluationResultSplit) -> Self {
        match s {
            EvaluationResultSplit::Full => Self::Full,
            EvaluationResultSplit::Tuning => Self::Tuning,
            EvaluationResultSplit::Validation => Self::Validation,
            EvaluationResultSplit::Holdout => Self::Holdout,
        }
    }
}

impl From<EvaluationResultSplitPayload> for EvaluationResultSplit {
    fn from(s: EvaluationResultSplitPayload) -> Self {
        match s {
            EvaluationResultSplitPayload::Full => Self::Full,
            EvaluationResultSplitPayload::Tuning => Self::Tuning,
            EvaluationResultSplitPayload::Validation => Self::Validation,
            EvaluationResultSplitPayload::Holdout => Self::Holdout,
        }
    }
}

impl From<EvaluationMetrics> for EvaluationMetricsPayload {
    fn from(m: EvaluationMetrics) -> Self {
        Self {
            recall_mean: m.recall_mean,
            recall_std: m.recall_std,
            precision_mean: m.precision_mean,
            precision_std: m.precision_std,
            iou_mean: m.iou_mean,
            iou_std: m.iou_std,
            precision_omega_mean: m.precision_omega_mean,
            precision_omega_std: m.precision_omega_std,
            chunk_count: m.chunk_count,
            average_chunk_tokens: m.average_chunk_tokens,
            average_retrieved_tokens: m.average_retrieved_tokens,
            recall_ci_low: m.recall_ci_low,
            recall_ci_high: m.recall_ci_high,
            precision_ci_low: m.precision_ci_low,
            precision_ci_high: m.precision_ci_high,
            iou_ci_low: m.iou_ci_low,
            iou_ci_high: m.iou_ci_high,
            precision_omega_ci_low: m.precision_omega_ci_low,
            precision_omega_ci_high: m.precision_omega_ci_high,
            composite_ci_low: m.composite_ci_low,
            composite_ci_high: m.composite_ci_high,
            judge_score: m.judge_score,
        }
    }
}

impl From<EvaluationMetricsPayload> for EvaluationMetrics {
    fn from(m: EvaluationMetricsPayload) -> Self {
        Self {
            recall_mean: m.recall_mean,
            recall_std: m.recall_std,
            precision_mean: m.precision_mean,
            precision_std: m.precision_std,
            iou_mean: m.iou_mean,
            iou_std: m.iou_std,
            precision_omega_mean: m.precision_omega_mean,
            precision_omega_std: m.precision_omega_std,
            chunk_count: m.chunk_count,
            average_chunk_tokens: m.average_chunk_tokens,
            average_retrieved_tokens: m.average_retrieved_tokens,
            recall_ci_low: m.recall_ci_low,
            recall_ci_high: m.recall_ci_high,
            precision_ci_low: m.precision_ci_low,
            precision_ci_high: m.precision_ci_high,
            iou_ci_low: m.iou_ci_low,
            iou_ci_high: m.iou_ci_high,
            precision_omega_ci_low: m.precision_omega_ci_low,
            precision_omega_ci_high: m.precision_omega_ci_high,
            composite_ci_low: m.composite_ci_low,
            composite_ci_high: m.composite_ci_high,
            judge_score: m.judge_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_metrics() -> EvaluationMetrics {
        EvaluationMetrics {
            recall_mean: 0.8,
            recall_std: 0.1,
            precision_mean: 0.7,
            precision_std: 0.1,
            iou_mean: 0.6,
            iou_std: 0.05,
            precision_omega_mean: 0.75,
            precision_omega_std: 0.1,
            chunk_count: 10,
            average_chunk_tokens: 200,
            average_retrieved_tokens: 250,
            recall_ci_low: 0.7,
            recall_ci_high: 0.9,
            precision_ci_low: 0.6,
            precision_ci_high: 0.8,
            iou_ci_low: 0.55,
            iou_ci_high: 0.65,
            precision_omega_ci_low: 0.7,
            precision_omega_ci_high: 0.8,
            composite_ci_low: 0.65,
            composite_ci_high: 0.85,
            judge_score: Some(0.9),
        }
    }

    #[test]
    fn metrics_round_trip() {
        let original = sample_metrics();
        let payload: EvaluationMetricsPayload = original.clone().into();
        let wire = serde_json::to_string(&payload).unwrap();
        let decoded: EvaluationMetricsPayload = serde_json::from_str(&wire).unwrap();
        let restored: EvaluationMetrics = decoded.into();
        assert_eq!(restored.recall_mean, original.recall_mean);
        assert_eq!(restored.judge_score, original.judge_score);
        assert_eq!(restored.composite_ci_high, original.composite_ci_high);
    }

    #[test]
    fn split_serializes_snake_case() {
        let payload: EvaluationResultSplitPayload = EvaluationResultSplit::Holdout.into();
        let wire = serde_json::to_value(&payload).unwrap();
        assert_eq!(wire, json!("holdout"));
    }

    #[test]
    fn optimization_config_round_trips() {
        let original = OptimizationConfig {
            budget: OptimizationBudget::Thorough,
            scope: OptimizationScope::Both,
            judges_enabled: true,
            seed: Some(42),
        };
        let payload: OptimizationConfigPayload = original.clone().into();
        let wire = serde_json::to_string(&payload).unwrap();
        let decoded: OptimizationConfigPayload = serde_json::from_str(&wire).unwrap();
        let restored: OptimizationConfig = decoded.into();
        assert_eq!(restored.budget, original.budget);
        assert_eq!(restored.scope, original.scope);
        assert_eq!(restored.judges_enabled, original.judges_enabled);
        assert_eq!(restored.seed, original.seed);
    }

    #[test]
    fn options_accept_string_encoded_numbers() {
        let legacy = json!({"top_k": "5", "min_score_milli": "300"});
        let payload: EvaluationRunOptionsPayload = serde_json::from_value(legacy).unwrap();
        assert_eq!(payload.top_k, 5);
        assert_eq!(payload.min_score_milli, 300);
    }
}
