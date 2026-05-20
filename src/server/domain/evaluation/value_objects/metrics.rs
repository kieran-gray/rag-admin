use serde::{Deserialize, Serialize};

use crate::shared::EvaluationMetrics as EvaluationMetricsDto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetrics {
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

impl From<EvaluationMetricsDto> for EvaluationMetrics {
    fn from(m: EvaluationMetricsDto) -> Self {
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

impl From<EvaluationMetrics> for EvaluationMetricsDto {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metrics() -> EvaluationMetricsDto {
        EvaluationMetricsDto {
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
        let domain: EvaluationMetrics = original.clone().into();
        let wire = serde_json::to_string(&domain).unwrap();
        let decoded: EvaluationMetrics = serde_json::from_str(&wire).unwrap();
        let restored: EvaluationMetricsDto = decoded.into();
        assert_eq!(restored.recall_mean, original.recall_mean);
        assert_eq!(restored.judge_score, original.judge_score);
        assert_eq!(restored.composite_ci_high, original.composite_ci_high);
    }
}
