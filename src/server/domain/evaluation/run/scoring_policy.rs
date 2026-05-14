use serde::{Deserialize, Serialize};

use crate::shared::EvaluationMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub recall: f32,
    pub iou: f32,
    pub precision: f32,
    pub precision_omega: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            recall: 0.40,
            iou: 0.25,
            precision: 0.20,
            precision_omega: 0.15,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoringPolicy {
    pub weights: ScoringWeights,
}

impl ScoringPolicy {
    pub fn score(self, metrics: &EvaluationMetrics) -> f32 {
        metrics.recall_mean * self.weights.recall
            + metrics.iou_mean * self.weights.iou
            + metrics.precision_mean * self.weights.precision
            + metrics.precision_omega_mean * self.weights.precision_omega
    }
}
