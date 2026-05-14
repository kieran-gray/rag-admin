use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::evaluation::question::EvaluationQuestion;

use super::read_model::{EvaluationDatasetReadModel, NewDatasetSummary};

#[derive(Debug, Error)]
pub enum EvaluationDatasetRepositoryError {
    #[error("evaluation dataset repository error: {0}")]
    Internal(String),
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

    async fn mark_failed(
        &self,
        dataset_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn rename(
        &self,
        dataset_id: Uuid,
        label: String,
    ) -> Result<(), EvaluationDatasetRepositoryError>;

    async fn mark_deleted(&self, dataset_id: Uuid) -> Result<(), EvaluationDatasetRepositoryError>;
}
