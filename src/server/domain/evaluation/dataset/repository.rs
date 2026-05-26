use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::domain::evaluation::question::EvaluationQuestion;

use super::read_model::{EvaluationDatasetReadModel, NewDatasetSummary};
use event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum EvaluationDatasetRepositoryError {
    #[error("evaluation dataset repository error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatasetStatusFilter {
    Completed,
    Generating,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct DatasetListCursor {
    pub created_at: OffsetDateTime,
    pub dataset_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct DatasetListQuery {
    pub cursor: Option<DatasetListCursor>,
    pub limit: u32,
    pub statuses: Vec<DatasetStatusFilter>,
}

#[derive(Debug, Clone)]
pub struct DatasetListPageItem {
    pub dataset: EvaluationDatasetReadModel,
    pub document_title: Option<String>,
    pub run_count: u32,
}

#[derive(Debug, Clone)]
pub struct DatasetListPage {
    pub items: Vec<DatasetListPageItem>,
    pub next_cursor: Option<DatasetListCursor>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<(DatasetStatusFilter, u64)>,
}

#[async_trait]
pub trait EvaluationDatasetRepository: Send + Sync {
    async fn load(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError>;

    async fn list_page(
        &self,
        query: &DatasetListQuery,
    ) -> Result<DatasetListPage, EvaluationDatasetRepositoryError>;

    async fn load_questions(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationQuestion>, EvaluationDatasetRepositoryError>;

    async fn insert_summary(
        &self,
        summary: NewDatasetSummary,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn save_question(
        &self,
        dataset_id: Uuid,
        question: EvaluationQuestion,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn increment_rejection_count(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn mark_completed(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn set_map_status(
        &self,
        dataset_id: Uuid,
        ready: bool,
        failure_reason: Option<String>,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn mark_failed(
        &self,
        dataset_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn mark_cancelled(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn rename(
        &self,
        dataset_id: Uuid,
        label: String,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn mark_deleted(&self, dataset_id: Uuid) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn increment_run_count(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn find_awaiting_for_map(
        &self,
        map_id: Uuid,
    ) -> Result<Vec<Uuid>, EvaluationDatasetRepositoryError>;
}

impl From<EvaluationDatasetRepositoryError> for ProjectionError {
    fn from(value: EvaluationDatasetRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
