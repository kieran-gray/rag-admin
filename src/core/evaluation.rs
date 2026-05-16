use serde::{Deserialize, Serialize};

use crate::core::chunking::ChunkingConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct OrderedF32(pub f32);

impl Eq for OrderedF32 {}

impl From<f32> for OrderedF32 {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<OrderedF32> for f32 {
    fn from(value: OrderedF32) -> Self {
        value.0
    }
}

pub fn ordered_f32_vec(values: Vec<f32>) -> Vec<OrderedF32> {
    values.into_iter().map(OrderedF32::from).collect()
}

pub fn plain_f32_vec(values: &[OrderedF32]) -> Vec<f32> {
    values.iter().copied().map(f32::from).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluationRunOptions {
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub top_k: u32,
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub min_score_milli: u32,
}

impl Default for EvaluationRunOptions {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            min_score_milli: 0,
        }
    }
}

impl EvaluationRunOptions {
    pub fn min_score(&self) -> f32 {
        milli_to_f32(self.min_score_milli)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationBudget {
    Quick,
    #[default]
    Thorough,
    Exhaustive,
}

impl OptimizationBudget {
    pub fn describe(self) -> &'static str {
        match self {
            OptimizationBudget::Quick => "24 + 12 + 6 trials · 3 rungs · holdout top 1",
            OptimizationBudget::Thorough => "48 + 24 + 12 + 6 trials · 4 rungs · holdout top 3",
            OptimizationBudget::Exhaustive => "96 + 48 + 24 + 12 trials · 4 rungs · holdout top 5",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationScope {
    Chunking,
    Retrieval,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OptimizationConfig {
    pub budget: OptimizationBudget,
    pub scope: OptimizationScope,
    pub judges_enabled: bool,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationAutotuneRequest {
    #[serde(
        default = "default_tuning_fraction_milli",
        deserialize_with = "crate::core::serde_compat::u32_from_string"
    )]
    pub tuning_fraction_milli: u32,
    #[serde(
        default = "default_holdout_top_n",
        deserialize_with = "crate::core::serde_compat::u32_from_string"
    )]
    pub holdout_top_n: u32,
}

impl Default for EvaluationAutotuneRequest {
    fn default() -> Self {
        Self {
            tuning_fraction_milli: default_tuning_fraction_milli(),
            holdout_top_n: default_holdout_top_n(),
        }
    }
}

impl EvaluationAutotuneRequest {
    pub fn tuning_fraction(&self) -> f32 {
        milli_to_f32(self.tuning_fraction_milli)
    }
}

fn default_tuning_fraction_milli() -> u32 {
    700
}

fn default_holdout_top_n() -> u32 {
    3
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityFlag {
    ValidationHoldoutGap,
    FlatLandscape,
    SmallSample,
    StatisticalTie,
}

impl ReliabilityFlag {
    pub fn headline(self) -> &'static str {
        match self {
            Self::ValidationHoldoutGap => "Holdout score diverges from selection score",
            Self::FlatLandscape => "Dataset isn't discriminating between configs",
            Self::SmallSample => "Small sample, wide confidence intervals",
            Self::StatisticalTie => {
                "Top configs may be tied (overlapping 95% confidence intervals)"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResultSplit {
    #[default]
    Full,
    Tuning,
    Validation,
    Holdout,
}

impl EvaluationResultSplit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Tuning => "tuning",
            Self::Validation => "validation",
            Self::Holdout => "holdout",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "full" => Ok(Self::Full),
            "tuning" => Ok(Self::Tuning),
            "validation" => Ok(Self::Validation),
            "holdout" => Ok(Self::Holdout),
            other => Err(format!("unknown evaluation split '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationAutotuneSummary {
    pub tuning_question_count: u32,
    pub holdout_question_count: u32,
    pub candidate_count: u32,
    pub selected_label: String,
    pub selected_options: EvaluationRunOptions,
    pub selected_config: ChunkingConfig,
    pub tuning_score: f32,
    pub holdout_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingVariant {
    pub label: String,
    pub config: ChunkingConfig,
}

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

impl EvaluationMetrics {
    pub fn composite_ci_half_width(&self) -> f32 {
        ((self.composite_ci_high - self.composite_ci_low) / 2.0).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluationScoreWeights {
    pub recall: f32,
    pub iou: f32,
    pub precision: f32,
    pub precision_omega: f32,
}

impl Default for EvaluationScoreWeights {
    fn default() -> Self {
        Self {
            recall: 0.40,
            iou: 0.25,
            precision: 0.20,
            precision_omega: 0.15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluationScorePolicy {
    weights: EvaluationScoreWeights,
}

impl EvaluationScorePolicy {
    pub fn weights(&self) -> EvaluationScoreWeights {
        self.weights
    }

    pub fn new(weights: EvaluationScoreWeights) -> Self {
        Self { weights }
    }

    pub fn score(self, metrics: &EvaluationMetrics) -> f32 {
        metrics.recall_mean * self.weights.recall
            + metrics.iou_mean * self.weights.iou
            + metrics.precision_mean * self.weights.precision
            + metrics.precision_omega_mean * self.weights.precision_omega
    }
}

impl Default for EvaluationScorePolicy {
    fn default() -> Self {
        Self::new(EvaluationScoreWeights::default())
    }
}

pub fn evaluation_score(metrics: &EvaluationMetrics) -> f32 {
    EvaluationScorePolicy::default().score(metrics)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReferenceResult {
    pub content: String,
    pub char_start: u32,
    pub char_end: u32,
    pub covered_chars: u32,
    pub total_chars: u32,
    pub recall: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationQuestionResult {
    pub question: String,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub retrieved_chunk_ids: Vec<u32>,
    pub missed_reference_count: u32,
    pub reference_results: Vec<EvaluationReferenceResult>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationVariantResult {
    pub variant: ChunkingVariant,
    #[serde(default)]
    pub options: EvaluationRunOptions,
    #[serde(default)]
    pub split: EvaluationResultSplit,
    #[serde(default)]
    pub selected: bool,
    pub metrics: EvaluationMetrics,
    pub question_results: Vec<EvaluationQuestionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunResult {
    #[serde(default)]
    pub run_id: String,
    pub slug: String,
    pub post_version: String,
    #[serde(default)]
    pub created_at: String,
    pub options: EvaluationRunOptions,
    #[serde(default)]
    pub autotune: Option<EvaluationAutotuneSummary>,
    pub variants: Vec<EvaluationVariantResult>,
}

impl EvaluationRunResult {
    pub fn new(
        slug: String,
        post_version: String,
        created_at: String,
        options: EvaluationRunOptions,
        autotune: Option<EvaluationAutotuneSummary>,
        variants: Vec<EvaluationVariantResult>,
    ) -> Self {
        Self {
            run_id: Self::run_id(),
            slug,
            post_version,
            created_at,
            options,
            autotune,
            variants,
        }
    }

    fn run_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("run-{nanos}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunSummary {
    pub run_id: String,
    pub created_at: String,
    pub options: EvaluationRunOptions,
    pub variant_labels: Vec<String>,
    pub variant_count: u32,
    pub option_count: u32,
    pub best_label: String,
    pub best_score: f32,
    pub best_recall: f32,
    pub best_precision: f32,
    pub best_precision_omega: f32,
}

fn milli_to_f32(value: u32) -> f32 {
    value as f32 / 1000.0
}

fn default_top_k() -> u32 {
    5
}
