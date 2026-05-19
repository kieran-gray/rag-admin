use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::{EvaluationRunReadModel, EvaluationVariantResultDto, NewRunSummary};
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
}

#[derive(Debug, Clone)]
pub struct RunListPage {
    pub items: Vec<EvaluationRunReadModel>,
    pub next_cursor: Option<RunListCursor>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<(RunStatusFilter, u64)>,
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
