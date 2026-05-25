use std::collections::HashMap;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::{EvaluationRunReadModel, EvaluationVariantResultDto, NewRunSummary};
use crate::server::domain::evaluation::value_objects::{
    EvaluationResultSplit, EvaluationRunOptions,
};
use event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum EvaluationRunRepositoryError {
    #[error("evaluation run repository error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunStatusFilter {
    Completed,
    Running,
    Failed,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunKindFilter {
    Optimization,
    Manual,
}

#[derive(Debug, Clone)]
pub struct RunListCursor {
    pub created_at: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct RunListQuery {
    pub cursor: Option<RunListCursor>,
    pub limit: u32,
    pub statuses: Vec<RunStatusFilter>,
    pub kinds: Vec<RunKindFilter>,
}

#[derive(Debug, Clone)]
pub struct RunListPage {
    pub items: Vec<EvaluationRunReadModel>,
    pub next_cursor: Option<RunListCursor>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<(RunStatusFilter, u64)>,
    pub kind_counts: Vec<(RunKindFilter, u64)>,
}

#[derive(Debug, Clone)]
pub struct QuestionResultRow {
    pub sequence: u32,
    pub question: String,
    pub category: String,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub retrieved_chunk_ids: Vec<Uuid>,
    pub scores: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct ChunkExcerpt {
    pub chunk_id: Uuid,
    pub sequence: u32,
    pub heading: String,
    pub text: String,
    pub char_start: u32,
    pub char_end: u32,
}

#[derive(Debug, Clone)]
pub struct ReferenceExcerpt {
    pub question_sequence: u32,
    pub sequence: u32,
    pub content: String,
    pub char_start: u32,
    pub char_end: u32,
}

#[derive(Debug, Clone)]
pub struct VariantSplitOption {
    pub variant_label: String,
    pub splits: Vec<EvaluationResultSplit>,
}

#[derive(Debug, Clone)]
pub struct RunQuestionResultsLoad {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub variant_label: String,
    pub split: EvaluationResultSplit,
    pub options: EvaluationRunOptions,
    pub questions: Vec<QuestionResultRow>,
    pub references: Vec<ReferenceExcerpt>,
    pub chunks: HashMap<Uuid, ChunkExcerpt>,
    pub variants: Vec<VariantSplitOption>,
}

#[async_trait]
pub trait EvaluationRunRepository: Send + Sync {
    async fn load(
        &self,
        run_id: Uuid,
    ) -> Result<Option<EvaluationRunReadModel>, EvaluationRunRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError>;

    async fn list_for_dataset(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError>;

    async fn list_recent(
        &self,
        limit: u32,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError>;

    async fn list_page(
        &self,
        query: &RunListQuery,
    ) -> Result<RunListPage, EvaluationRunRepositoryError>;

    async fn load_variant_results(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<EvaluationVariantResultDto>, EvaluationRunRepositoryError>;

    async fn load_question_results(
        &self,
        run_id: Uuid,
        variant_label: &str,
        split: Option<EvaluationResultSplit>,
    ) -> Result<Option<RunQuestionResultsLoad>, EvaluationRunRepositoryError>;

    async fn insert_summary(
        &self,
        summary: NewRunSummary,
    ) -> Result<(), EvaluationRunRepositoryError>;

    async fn record_variant_prepared(
        &self,
        run_id: Uuid,
    ) -> Result<(), EvaluationRunRepositoryError>;

    async fn save_variant_result(
        &self,
        result: EvaluationVariantResultDto,
    ) -> Result<(), EvaluationRunRepositoryError>;

    async fn mark_completed(&self, run_id: Uuid) -> Result<(), EvaluationRunRepositoryError>;

    async fn mark_failed(
        &self,
        run_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationRunRepositoryError>;
}

impl From<EvaluationRunRepositoryError> for ProjectionError {
    fn from(value: EvaluationRunRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
