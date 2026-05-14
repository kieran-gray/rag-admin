use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::evaluation::{
    dataset::{read_model::EvaluationDatasetReadModel, repository::EvaluationDatasetRepository},
    run::{
        read_model::{EvaluationRunReadModel, EvaluationVariantResultDto},
        repository::EvaluationRunRepository,
    },
};

pub struct EvaluationQueryService {
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    run_repository: Arc<dyn EvaluationRunRepository>,
}

impl EvaluationQueryService {
    pub fn new(
        dataset_repository: Arc<dyn EvaluationDatasetRepository>,
        run_repository: Arc<dyn EvaluationRunRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            dataset_repository,
            run_repository,
        })
    }

    pub async fn get_dataset(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<EvaluationDatasetReadModel>, AppError> {
        self.dataset_repository
            .load(dataset_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load evaluation dataset: {e}")))
    }

    pub async fn list_datasets_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationDatasetReadModel>, AppError> {
        self.dataset_repository
            .list_for_document(document_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list evaluation datasets: {e}")))
    }

    pub async fn load_questions(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<crate::server::domain::evaluation::question::EvaluationQuestion>, AppError>
    {
        self.dataset_repository
            .load_questions(dataset_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load evaluation questions: {e}")))
    }

    pub async fn get_run(&self, run_id: Uuid) -> Result<Option<EvaluationRunReadModel>, AppError> {
        let Some(mut run) = self
            .run_repository
            .load(run_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load evaluation run: {e}")))?
        else {
            return Ok(None);
        };

        run.variant_results = self.load_variant_results(run_id).await?;
        Ok(Some(run))
    }

    pub async fn list_runs_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, AppError> {
        self.run_repository
            .list_for_document(document_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list evaluation runs: {e}")))
    }

    pub async fn list_runs_for_dataset(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, AppError> {
        self.run_repository
            .list_for_dataset(dataset_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list evaluation runs: {e}")))
    }

    async fn load_variant_results(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<EvaluationVariantResultDto>, AppError> {
        self.run_repository
            .load_variant_results(run_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load variant results: {e}")))
    }
}
