use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::contracts::comprehension::MapItemRefDto;
use crate::shared::{
    ChunkingConfig, EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
    EvaluationScoreWeights, EvaluationVariantResult, OptimizationConfig, OrderedF32,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationReferenceDto {
    pub document_id: Uuid,
    pub content: String,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_start: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub char_end: u32,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionDimensionsDto {
    pub operation: String,
    pub evidence: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxisWeightDto {
    pub operation: String,
    pub evidence: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationQuestionDto {
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReferenceDto>,
    #[serde(default)]
    pub embedding: Option<Vec<OrderedF32>>,
    pub dimensions: QuestionDimensionsDto,
    pub evidence_refs: Vec<MapItemRefDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationJobInfo {
    pub job_id: String,
    pub stream_url: String,
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
pub struct DatasetListItemDto {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_title: Option<String>,
    pub label: String,
    pub status: String,
    pub failure_reason: Option<String>,
    pub target_question_count: u32,
    pub question_count: u32,
    pub rejection_count: u32,
    pub run_count: u32,
    pub generation_model: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatusFilterDto {
    Completed,
    Generating,
    Failed,
    Cancelled,
}

impl DatasetStatusFilterDto {
    pub fn label(self) -> &'static str {
        match self {
            Self::Completed => "Completed",
            Self::Generating => "Generating",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Generating => "generating",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "completed" => Some(Self::Completed),
            "generating" => Some(Self::Generating),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetListQueryDto {
    #[serde(default)]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(default)]
    pub statuses: Vec<DatasetStatusFilterDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetStatusFacetDto {
    pub status: DatasetStatusFilterDto,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetListPageDto {
    pub items: Vec<DatasetListItemDto>,
    pub next_cursor: Option<String>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<DatasetStatusFacetDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetDto {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_title: Option<String>,
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
pub struct EvaluationRunDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub status: String,
    pub variants: Vec<EvaluationVariantResult>,
    pub created_at: String,
    #[serde(default = "RunKindDto::default_manual")]
    pub kind: RunKindDto,
    #[serde(default)]
    pub optimization: Option<OptimizationConfig>,
    #[serde(default)]
    pub document_id: Option<Uuid>,
    #[serde(default)]
    pub document_title: Option<String>,
    #[serde(default)]
    pub variant_count: u32,
    #[serde(default)]
    pub variants_scored: u32,
    #[serde(default)]
    pub scoring_weights: EvaluationScoreWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestVariantDto {
    pub label: String,
    pub config: ChunkingConfig,
    pub options: EvaluationRunOptions,
    pub score: f32,
    pub metrics: EvaluationMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEvaluationRunDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_title: Option<String>,
    pub status: String,
    pub variant_count: u32,
    #[serde(default)]
    pub variants_scored: u32,
    #[serde(default)]
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub best: Option<BestVariantDto>,
    #[serde(default = "RunKindDto::default_manual")]
    pub kind: RunKindDto,
    #[serde(default)]
    pub optimization: Option<OptimizationConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKindDto {
    Optimization,
    Manual,
}

impl RunKindDto {
    pub fn label(self) -> &'static str {
        match self {
            RunKindDto::Optimization => "Optimization",
            RunKindDto::Manual => "Manual",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            RunKindDto::Optimization => "optimization",
            RunKindDto::Manual => "manual",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "optimization" => Some(RunKindDto::Optimization),
            "manual" => Some(RunKindDto::Manual),
            _ => None,
        }
    }

    pub fn from_optimization(opt: Option<&OptimizationConfig>) -> Self {
        match opt {
            Some(_) => RunKindDto::Optimization,
            None => RunKindDto::Manual,
        }
    }

    fn default_manual() -> Self {
        RunKindDto::Manual
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunKindFacetDto {
    pub kind: RunKindDto,
    pub count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatusFilterDto {
    Completed,
    Running,
    Failed,
    Pending,
}

impl RunStatusFilterDto {
    pub fn label(self) -> &'static str {
        match self {
            RunStatusFilterDto::Completed => "Completed",
            RunStatusFilterDto::Running => "Running",
            RunStatusFilterDto::Failed => "Failed",
            RunStatusFilterDto::Pending => "Pending",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            RunStatusFilterDto::Completed => "completed",
            RunStatusFilterDto::Running => "running",
            RunStatusFilterDto::Failed => "failed",
            RunStatusFilterDto::Pending => "pending",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "completed" => Some(RunStatusFilterDto::Completed),
            "running" => Some(RunStatusFilterDto::Running),
            "failed" => Some(RunStatusFilterDto::Failed),
            "pending" => Some(RunStatusFilterDto::Pending),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunListQueryDto {
    #[serde(default)]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(default)]
    pub statuses: Vec<RunStatusFilterDto>,
    #[serde(default)]
    pub kinds: Vec<RunKindDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatusFacetDto {
    pub status: RunStatusFilterDto,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunListPageDto {
    pub items: Vec<RecentEvaluationRunDto>,
    pub next_cursor: Option<String>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<RunStatusFacetDto>,
    #[serde(default)]
    pub kind_counts: Vec<RunKindFacetDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunQuestionVariantOptionDto {
    pub variant_label: String,
    pub splits: Vec<EvaluationResultSplit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunkDto {
    pub chunk_id: Uuid,
    pub rank: u32,
    pub score: f32,
    pub sequence: u32,
    pub heading: String,
    pub text: String,
    pub char_start: u32,
    pub char_end: u32,
    pub is_reference_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceExcerptDto {
    pub sequence: u32,
    pub content: String,
    pub char_start: u32,
    pub char_end: u32,
    pub covered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunQuestionResultDto {
    pub sequence: u32,
    pub question: String,
    pub operation: String,
    pub evidence_kind: String,
    pub role: String,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub retrieved: Vec<RetrievedChunkDto>,
    pub references: Vec<ReferenceExcerptDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunQuestionResultsDto {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub variant_label: String,
    pub split: EvaluationResultSplit,
    pub options: EvaluationRunOptions,
    pub variants: Vec<RunQuestionVariantOptionDto>,
    pub questions: Vec<RunQuestionResultDto>,
}
