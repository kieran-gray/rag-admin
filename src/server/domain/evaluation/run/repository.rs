use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::{EvaluationRunReadModel, EvaluationVariantResultDto, NewRunSummary};

#[derive(Debug, Error)]
pub enum EvaluationRunRepositoryError {
    #[error("evaluation run repository error: {0}")]
    Internal(String),
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
