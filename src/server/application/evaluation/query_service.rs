use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::evaluation::question::EvaluationQuestion;
use crate::server::domain::evaluation::{
    dataset::{
        read_model::EvaluationDatasetReadModel,
        repository::{
            DatasetListCursor, DatasetListPage, DatasetListQuery, DatasetStatusFilter,
            EvaluationDatasetRepository,
        },
    },
    run::{
        read_model::{EvaluationRunReadModel, EvaluationVariantResultDto},
        repository::{EvaluationRunRepository, RunListPage, RunListQuery},
    },
};
use crate::shared::contracts::{
    DatasetListItemDto, DatasetListPageDto, DatasetListQueryDto, DatasetStatusFacetDto,
    DatasetStatusFilterDto,
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

    pub async fn list_datasets_page(
        &self,
        query: DatasetListQueryDto,
    ) -> Result<DatasetListPageDto, AppError> {
        let cursor = match query.cursor.as_deref() {
            Some(raw) => Some(parse_dataset_cursor(raw)?),
            None => None,
        };
        let statuses = query
            .statuses
            .into_iter()
            .map(dataset_filter_dto_to_domain)
            .collect();
        let domain_query = DatasetListQuery {
            cursor,
            limit: query.limit,
            statuses,
        };
        let page: DatasetListPage = self
            .dataset_repository
            .list_page(&domain_query)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list datasets page: {e}")))?;

        Ok(DatasetListPageDto {
            items: page
                .items
                .into_iter()
                .map(|item| DatasetListItemDto {
                    dataset_id: item.dataset.dataset_id,
                    document_id: item.dataset.document_id,
                    document_title: item.document_title,
                    label: item.dataset.label.clone(),
                    status: item.dataset.status.as_str().to_string(),
                    failure_reason: item.dataset.failure_reason.clone(),
                    target_question_count: item.dataset.target_question_count,
                    question_count: item.dataset.question_count,
                    rejection_count: item.dataset.rejection_count,
                    run_count: item.run_count,
                    generation_model: item.dataset.generation_model.clone(),
                    created_at: item.dataset.created_at.to_string(),
                })
                .collect(),
            next_cursor: page.next_cursor.map(|c| encode_dataset_cursor(&c)),
            total_matching: page.total_matching,
            total_all: page.total_all,
            status_counts: page
                .status_counts
                .into_iter()
                .map(|(status, count)| DatasetStatusFacetDto {
                    status: dataset_filter_domain_to_dto(status),
                    count,
                })
                .collect(),
        })
    }

    pub async fn load_questions(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationQuestion>, AppError> {
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

    pub async fn list_recent_runs(
        &self,
        limit: u32,
    ) -> Result<Vec<EvaluationRunReadModel>, AppError> {
        let mut runs = self
            .run_repository
            .list_recent(limit)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list recent runs: {e}")))?;

        for run in &mut runs {
            run.variant_results = self.load_variant_results(run.run_id).await?;
        }
        Ok(runs)
    }

    pub async fn list_runs_page(&self, query: RunListQuery) -> Result<RunListPage, AppError> {
        let mut page = self
            .run_repository
            .list_page(&query)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list runs page: {e}")))?;

        for run in &mut page.items {
            run.variant_results = self.load_variant_results(run.run_id).await?;
        }
        Ok(page)
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

fn dataset_filter_dto_to_domain(status: DatasetStatusFilterDto) -> DatasetStatusFilter {
    match status {
        DatasetStatusFilterDto::Completed => DatasetStatusFilter::Completed,
        DatasetStatusFilterDto::Generating => DatasetStatusFilter::Generating,
        DatasetStatusFilterDto::Failed => DatasetStatusFilter::Failed,
        DatasetStatusFilterDto::Cancelled => DatasetStatusFilter::Cancelled,
    }
}

fn dataset_filter_domain_to_dto(status: DatasetStatusFilter) -> DatasetStatusFilterDto {
    match status {
        DatasetStatusFilter::Completed => DatasetStatusFilterDto::Completed,
        DatasetStatusFilter::Generating => DatasetStatusFilterDto::Generating,
        DatasetStatusFilter::Failed => DatasetStatusFilterDto::Failed,
        DatasetStatusFilter::Cancelled => DatasetStatusFilterDto::Cancelled,
    }
}

fn encode_dataset_cursor(cursor: &DatasetListCursor) -> String {
    format!("{}|{}", cursor.created_at, cursor.dataset_id)
}

fn parse_dataset_cursor(raw: &str) -> Result<DatasetListCursor, AppError> {
    let (created_at, id) = raw
        .split_once('|')
        .ok_or_else(|| AppError::Validation(format!("invalid dataset cursor: {raw}")))?;
    let dataset_id = Uuid::parse_str(id)
        .map_err(|e| AppError::Validation(format!("invalid dataset cursor uuid: {e}")))?;
    Ok(DatasetListCursor {
        created_at: created_at.to_string(),
        dataset_id,
    })
}
