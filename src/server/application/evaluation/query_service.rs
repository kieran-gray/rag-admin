use std::collections::HashMap;
use std::sync::Arc;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::evaluation::question::EvaluationQuestion;
use crate::server::domain::evaluation::value_objects::EvaluationResultSplit as DomainSplit;
use crate::server::domain::evaluation::{
    dataset::{
        read_model::EvaluationDatasetReadModel,
        repository::{
            DatasetListCursor, DatasetListPage, DatasetListQuery, DatasetStatusFilter,
            DatasetWithDocumentTitle, EvaluationDatasetRepository,
        },
    },
    run::{
        read_model::{EvaluationRunReadModel, EvaluationVariantResultRow},
        repository::{
            ChunkExcerpt, EvaluationRunRepository, ReferenceExcerpt, RunListPage, RunListQuery,
            RunQuestionResultsLoad,
        },
    },
};
use crate::shared::contracts::{
    DatasetListItemDto, DatasetListPageDto, DatasetListQueryDto, DatasetStatusFacetDto,
    DatasetStatusFilterDto, ReferenceExcerptDto, RetrievedChunkDto, RunQuestionResultDto,
    RunQuestionResultsDto, RunQuestionVariantOptionDto,
};
use crate::shared::EvaluationResultSplit as SplitDto;

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

    pub async fn get_dataset_with_document_title(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<DatasetWithDocumentTitle>, AppError> {
        self.dataset_repository
            .load_with_document_title(dataset_id)
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
        self.run_repository
            .list_recent(limit)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list recent runs: {e}")))
    }

    pub async fn list_runs_page(&self, query: RunListQuery) -> Result<RunListPage, AppError> {
        self.run_repository
            .list_page(&query)
            .await
            .map_err(|e| AppError::Internal(format!("failed to list runs page: {e}")))
    }

    async fn load_variant_results(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<EvaluationVariantResultRow>, AppError> {
        self.run_repository
            .load_variant_results(run_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load variant results: {e}")))
    }

    pub async fn get_run_question_results(
        &self,
        run_id: Uuid,
        variant_label: &str,
        split: Option<SplitDto>,
    ) -> Result<Option<RunQuestionResultsDto>, AppError> {
        let domain_split = split.map(DomainSplit::from);
        let load = self
            .run_repository
            .load_question_results(run_id, variant_label, domain_split)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load question results: {e}")))?;
        Ok(load.map(build_question_results_dto))
    }
}

fn build_question_results_dto(load: RunQuestionResultsLoad) -> RunQuestionResultsDto {
    let mut refs_by_question: HashMap<u32, Vec<&ReferenceExcerpt>> = HashMap::new();
    for r in &load.references {
        refs_by_question
            .entry(r.question_sequence)
            .or_default()
            .push(r);
    }

    let questions = load
        .questions
        .iter()
        .map(|q| {
            let empty: Vec<&ReferenceExcerpt> = Vec::new();
            let refs_for = refs_by_question.get(&q.sequence).unwrap_or(&empty);

            let mut retrieved: Vec<RetrievedChunkDto> = q
                .retrieved_chunk_ids
                .iter()
                .enumerate()
                .map(|(rank, chunk_id)| {
                    let score = q.scores.get(rank).copied().unwrap_or(0.0);
                    let excerpt = load.chunks.get(chunk_id);
                    let (sequence, heading, text, char_start, char_end) = match excerpt {
                        Some(c) => (
                            c.sequence,
                            c.heading.clone(),
                            c.text.clone(),
                            c.char_start,
                            c.char_end,
                        ),
                        None => (0, String::new(), String::new(), 0, 0),
                    };
                    let is_reference_hit =
                        excerpt.is_some_and(|c| refs_for.iter().any(|r| spans_overlap(c, r)));
                    RetrievedChunkDto {
                        chunk_id: *chunk_id,
                        rank: rank as u32,
                        score,
                        sequence,
                        heading,
                        text,
                        char_start,
                        char_end,
                        is_reference_hit,
                    }
                })
                .collect();
            retrieved.sort_by_key(|c| c.rank);

            let references = refs_for
                .iter()
                .map(|r| {
                    let covered = q
                        .retrieved_chunk_ids
                        .iter()
                        .any(|id| load.chunks.get(id).is_some_and(|c| spans_overlap(c, r)));
                    ReferenceExcerptDto {
                        sequence: r.sequence,
                        content: r.content.clone(),
                        char_start: r.char_start,
                        char_end: r.char_end,
                        covered,
                    }
                })
                .collect();

            RunQuestionResultDto {
                sequence: q.sequence,
                question: q.question.clone(),
                operation: q.operation.clone(),
                evidence_kind: q.evidence_kind.clone(),
                role: q.role.clone(),
                recall: q.recall,
                precision: q.precision,
                iou: q.iou,
                retrieved,
                references,
            }
        })
        .collect();

    RunQuestionResultsDto {
        run_id: load.run_id,
        dataset_id: load.dataset_id,
        variant_label: load.variant_label,
        split: SplitDto::from(load.split),
        options: load.options.into(),
        variants: load
            .variants
            .into_iter()
            .map(|v| RunQuestionVariantOptionDto {
                variant_label: v.variant_label,
                splits: v.splits.into_iter().map(SplitDto::from).collect(),
            })
            .collect(),
        questions,
    }
}

fn spans_overlap(chunk: &ChunkExcerpt, reference: &ReferenceExcerpt) -> bool {
    chunk.char_start < reference.char_end && reference.char_start < chunk.char_end
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
    let created_at = cursor
        .created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::new());
    format!("{}|{}", created_at, cursor.dataset_id)
}

fn parse_dataset_cursor(raw: &str) -> Result<DatasetListCursor, AppError> {
    let (created_at, id) = raw
        .split_once('|')
        .ok_or_else(|| AppError::Validation(format!("invalid dataset cursor: {raw}")))?;
    let dataset_id = Uuid::parse_str(id)
        .map_err(|e| AppError::Validation(format!("invalid dataset cursor uuid: {e}")))?;
    let created_at = OffsetDateTime::parse(created_at, &Rfc3339)
        .map_err(|e| AppError::Validation(format!("invalid dataset cursor timestamp: {e}")))?;
    Ok(DatasetListCursor {
        created_at,
        dataset_id,
    })
}
