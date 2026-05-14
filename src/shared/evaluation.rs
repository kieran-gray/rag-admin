use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ChunkingConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationGenerationBackend {
    #[default]
    Ollama,
    WorkersAi,
}

impl EvaluationGenerationBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::WorkersAi => "workers_ai",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationSettings {
    #[serde(default)]
    pub generation_backend: EvaluationGenerationBackend,
    #[serde(default = "default_generation_model")]
    pub generation_model: String,
    #[serde(
        default = "default_question_count",
        deserialize_with = "crate::shared::serde_compat::u32_from_string"
    )]
    pub question_count: u32,
    #[serde(
        default = "default_excerpt_similarity_threshold_milli",
        deserialize_with = "crate::shared::serde_compat::u32_from_string"
    )]
    pub excerpt_similarity_threshold_milli: u32,
    #[serde(
        default = "default_duplicate_similarity_threshold_milli",
        deserialize_with = "crate::shared::serde_compat::u32_from_string"
    )]
    pub duplicate_similarity_threshold_milli: u32,
    #[serde(
        default = "default_top_k",
        deserialize_with = "crate::shared::serde_compat::u32_from_string"
    )]
    pub top_k: u32,
    #[serde(
        default,
        deserialize_with = "crate::shared::serde_compat::u32_from_string"
    )]
    pub min_score_milli: u32,
}

impl Default for EvaluationSettings {
    fn default() -> Self {
        Self {
            generation_backend: EvaluationGenerationBackend::Ollama,
            generation_model: default_generation_model(),
            question_count: default_question_count(),
            excerpt_similarity_threshold_milli: default_excerpt_similarity_threshold_milli(),
            duplicate_similarity_threshold_milli: default_duplicate_similarity_threshold_milli(),
            top_k: default_top_k(),
            min_score_milli: 0,
        }
    }
}

impl EvaluationSettings {
    pub fn excerpt_similarity_threshold(&self) -> f32 {
        milli_to_f32(self.excerpt_similarity_threshold_milli)
    }

    pub fn duplicate_similarity_threshold(&self) -> f32 {
        milli_to_f32(self.duplicate_similarity_threshold_milli)
    }

    pub fn min_score(&self) -> f32 {
        milli_to_f32(self.min_score_milli)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationReferenceDto {
    pub content: String,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_start: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_end: u32,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationQuestionDto {
    pub question: String,
    pub references: Vec<EvaluationReferenceDto>,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationJobInfo {
    pub job_id: String,
    pub stream_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluationRunOptions {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub top_k: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationAutotuneRequest {
    pub current_config: ChunkingConfig,
    pub top_k_values: Vec<u32>,
    pub min_score_milli_values: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResultSplit {
    #[default]
    Full,
    Tuning,
    Holdout,
}

impl EvaluationResultSplit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Tuning => "tuning",
            Self::Holdout => "holdout",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "full" => Ok(Self::Full),
            "tuning" => Ok(Self::Tuning),
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

impl EvaluationRunOptions {
    pub fn min_score(&self) -> f32 {
        milli_to_f32(self.min_score_milli)
    }
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
    #[serde(default)]
    pub reference_results: Vec<EvaluationReferenceResult>,
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
    pub chunk_count: u32,
    #[serde(default, alias = "average_chunk_chars")]
    pub average_chunk_tokens: u32,
    #[serde(default, alias = "average_retrieved_chars")]
    pub average_retrieved_tokens: u32,
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

fn default_generation_model() -> String {
    "granite4.1:8b".into()
}

fn default_question_count() -> u32 {
    8
}

fn default_excerpt_similarity_threshold_milli() -> u32 {
    360
}

fn default_duplicate_similarity_threshold_milli() -> u32 {
    700
}

fn default_top_k() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetSummaryDto {
    pub dataset_id: Uuid,
    pub label: String,
    pub question_count: u32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetDto {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub label: String,
    pub status: String,
    pub target_question_count: u32,
    pub question_count: u32,
    pub rejection_count: u32,
    pub generation_model_id: Uuid,
    pub generation_model: String,
    pub embedding_model_id: Uuid,
    pub failure_reason: Option<String>,
    pub questions: Vec<EvaluationQuestionDto>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunSummaryDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub status: String,
    pub variant_count: u32,
    pub created_at: String,
}

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
pub struct EvaluationRunDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub status: String,
    pub variants: Vec<EvaluationVariantResult>,
    pub created_at: String,
}
