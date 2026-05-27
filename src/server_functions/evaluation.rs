use leptos::prelude::*;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::dataset::events::AxisWeight;
#[cfg(feature = "ssr")]
use crate::shared::contracts::BestVariantDto;
use crate::shared::contracts::{
    AxisWeightDto, DatasetListPageDto, DatasetListQueryDto, EvaluationDatasetDto,
    EvaluationDatasetSummaryDto, EvaluationJobInfo, EvaluationRunDto, EvaluationRunSummaryDto,
    RecentEvaluationRunDto, RunEvaluationRequestDto, RunListPageDto, RunListQueryDto,
    RunOptimizationRequestDto, RunQuestionResultsDto,
};
#[cfg(feature = "ssr")]
use crate::shared::contracts::{
    RunKindDto, RunKindFacetDto, RunStatusFacetDto, RunStatusFilterDto,
};
use crate::shared::EvaluationResultSplit;
#[cfg(feature = "ssr")]
use crate::shared::EvaluationScoreWeights;

#[cfg(feature = "ssr")]
use crate::server::application::evaluation::StartDatasetGenerationRequest;
#[cfg(feature = "ssr")]
use crate::server::application::source_document::SourceDocumentQueryService;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::aggregate::EvaluationRunStatus;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::read_model::EvaluationRunReadModel;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::repository::{
    RunKindFilter, RunListCursor, RunListQuery, RunStatusFilter,
};
#[cfg(feature = "ssr")]
use crate::server::setup::compose::evaluation::EvaluationServices;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::ingestion::IngestionServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::cmp::Ordering;
#[cfg(feature = "ssr")]
use std::collections::HashMap;
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(
    name = GetDatasetsForDocument,
    prefix = "/api",
    endpoint = "get_datasets_for_document"
)]
pub async fn get_datasets_for_document(
    document_id: Uuid,
) -> Result<Vec<EvaluationDatasetSummaryDto>, ServerFnError> {
    let datasets = ctx::<Arc<EvaluationServices>>()?
        .evaluation_query_service
        .list_datasets_for_document(document_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    Ok(datasets
        .into_iter()
        .map(|d| EvaluationDatasetSummaryDto {
            dataset_id: d.dataset_id,
            label: d.label,
            question_count: d.question_count,
            status: d.status.as_str().to_string(),
            created_at: d.created_at.to_string(),
        })
        .collect())
}

#[server(
    name = GetEvaluationDatasetsPage,
    prefix = "/api",
    endpoint = "get_evaluation_datasets_page"
)]
pub async fn get_evaluation_datasets_page(
    query: DatasetListQueryDto,
) -> Result<DatasetListPageDto, ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_query_service
        .list_datasets_page(query)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = GetDataset, prefix = "/api", endpoint = "get_dataset")]
pub async fn get_dataset(dataset_id: Uuid) -> Result<Option<EvaluationDatasetDto>, ServerFnError> {
    let services = ctx::<Arc<EvaluationServices>>()?;
    let query = &services.evaluation_query_service;

    let dataset = query
        .get_dataset_with_document_title(dataset_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    if let Some(loaded) = dataset {
        let questions = query
            .load_questions(dataset_id)
            .await
            .map_err(|e| map_app_error(&e))?;

        let d = loaded.dataset;
        Ok(Some(EvaluationDatasetDto {
            dataset_id: d.dataset_id,
            document_id: d.document_id,
            document_title: loaded.document_title,
            document_version: d.document_version,
            content_hash: d.content_hash,
            label: d.label,
            status: d.status.as_str().to_string(),
            target_question_count: d.target_question_count,
            question_count: d.question_count,
            rejection_count: d.rejection_count,
            generation_model_id: d.generation_model_id,
            generation_model: d.generation_model,
            embedding_model_id: d.embedding_model_id,
            failure_reason: d.failure_reason,
            questions: questions.into_iter().map(Into::into).collect(),
            created_at: d.created_at.to_string(),
        }))
    } else {
        Ok(None)
    }
}

#[server(
    name = StartGenerateSyntheticDataset,
    prefix = "/api",
    endpoint = "start_generate_synthetic_dataset"
)]
pub async fn start_generate_synthetic_dataset(
    document_id: Uuid,
    generation_model_id: Uuid,
    embedding_model_id: Uuid,
    label: String,
    question_count: u32,
    duplicate_similarity_threshold_milli: u32,
    weight_matrix: Vec<AxisWeightDto>,
) -> Result<EvaluationJobInfo, ServerFnError> {
    let weight_matrix: Vec<AxisWeight> = weight_matrix
        .into_iter()
        .map(|w| AxisWeight {
            operation: w.operation,
            evidence: w.evidence,
            weight: w.weight,
        })
        .collect();
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_dataset_command_handler
        .start_generation(StartDatasetGenerationRequest {
            document_id,
            generation_model_id,
            embedding_model_id,
            label,
            question_count,
            duplicate_similarity_threshold_milli,
            weight_matrix,
        })
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = RenameDataset, prefix = "/api", endpoint = "rename_dataset")]
pub async fn rename_dataset(dataset_id: Uuid, label: String) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_dataset_command_handler
        .rename(dataset_id, label)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = DeleteDataset, prefix = "/api", endpoint = "delete_dataset")]
pub async fn delete_dataset(dataset_id: Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_dataset_command_handler
        .delete(dataset_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = CancelDatasetGeneration, prefix = "/api", endpoint = "cancel_dataset_generation")]
pub async fn cancel_dataset_generation(dataset_id: Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_dataset_command_handler
        .cancel_generation(dataset_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = StartRunOptimization,
    prefix = "/api",
    endpoint = "start_run_optimization"
)]
pub async fn start_run_optimization(
    request: RunOptimizationRequestDto,
) -> Result<EvaluationJobInfo, ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_run_command_handler
        .start_optimization(request)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = StartRunEvaluation,
    prefix = "/api",
    endpoint = "start_run_evaluation"
)]
pub async fn start_run_evaluation(
    request: RunEvaluationRequestDto,
) -> Result<EvaluationJobInfo, ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_run_command_handler
        .start_run(request)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetRunsForDocument,
    prefix = "/api",
    endpoint = "get_runs_for_document"
)]
pub async fn get_runs_for_document(
    document_id: Uuid,
) -> Result<Vec<EvaluationRunSummaryDto>, ServerFnError> {
    let runs = ctx::<Arc<EvaluationServices>>()?
        .evaluation_query_service
        .list_runs_for_document(document_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    Ok(runs
        .into_iter()
        .map(|r| EvaluationRunSummaryDto {
            run_id: r.run_id,
            dataset_id: r.dataset_id,
            status: r.status.as_str().to_string(),
            variant_count: r.variants_count,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[server(
    name = ReplicateOptimizationRun,
    prefix = "/api",
    endpoint = "replicate_optimization_run"
)]
pub async fn replicate_optimization_run(run_id: Uuid) -> Result<Uuid, ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_run_command_handler
        .replicate_optimization(run_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = PromoteVariantToChunkingConfig,
    prefix = "/api",
    endpoint = "promote_variant_to_chunking_config"
)]
pub async fn promote_variant_to_chunking_config(
    run_id: Uuid,
    variant_label: String,
    name: String,
) -> Result<Uuid, ServerFnError> {
    use crate::server::setup::compose::catalog::CatalogServices;
    use crate::shared::contracts::{
        ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    };

    let trimmed_label = variant_label.trim();
    if trimmed_label.is_empty() {
        return Err(ServerFnError::new("variant_label is required".to_string()));
    }

    let services = ctx::<Arc<EvaluationServices>>()?;
    let query = &services.evaluation_query_service;
    let catalog = ctx::<Arc<CatalogServices>>()?;
    let chunking_handler = &catalog.chunking_configuration_command_handler;

    let run = query
        .get_run(run_id)
        .await
        .map_err(|e| map_app_error(&e))?
        .ok_or_else(|| ServerFnError::new(format!("run {run_id} not found")))?;

    let chosen = run
        .variant_results
        .iter()
        .find(|v| v.variant_label == trimmed_label)
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "run {run_id} has no variant labelled '{trimmed_label}'"
            ))
        })?;

    let trimmed_name = name.trim();
    let chosen_name = if trimmed_name.is_empty() {
        format!(
            "{}-{}",
            run_id.to_string().chars().take(8).collect::<String>(),
            chosen.variant_label
        )
    } else {
        trimmed_name.to_string()
    };

    let cmd = ChunkingConfigurationCommandDto::CreateChunkingConfiguration(
        CreateChunkingConfigurationDto {
            name: chosen_name,
            config: chosen.variant_config.into(),
            is_default: false,
        },
    );
    chunking_handler
        .handle_dto(cmd)
        .await
        .map_err(|e| map_app_error(&e))?;

    Ok(run_id)
}

#[server(name = GetRun, prefix = "/api", endpoint = "get_run")]
pub async fn get_run(run_id: Uuid) -> Result<Option<EvaluationRunDto>, ServerFnError> {
    let run = ctx::<Arc<EvaluationServices>>()?
        .evaluation_query_service
        .get_run(run_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    let ingestion = ctx::<Arc<IngestionServices>>()?;
    let documents = &ingestion.source_document_query_service;
    let doc_index = build_doc_index(documents).await?;

    Ok(run.map(|r| {
        let optimization_dto = r.optimization.clone().map(Into::into);
        let kind = RunKindDto::from_optimization(optimization_dto.as_ref());
        let document_title = doc_index.get(&r.document_id).cloned();
        let scoring_weights = EvaluationScoreWeights {
            recall: r.scoring_policy.weights.recall,
            iou: r.scoring_policy.weights.iou,
            precision: r.scoring_policy.weights.precision,
            precision_omega: r.scoring_policy.weights.precision_omega,
        };
        EvaluationRunDto {
            run_id: r.run_id,
            dataset_id: r.dataset_id,
            status: r.status.as_str().to_string(),
            variants: r.variant_results.into_iter().map(Into::into).collect(),
            created_at: r.created_at.to_string(),
            kind,
            optimization: optimization_dto,
            document_id: Some(r.document_id),
            document_title,
            variant_count: r.variants_count,
            variants_scored: r.variants_scored,
            scoring_weights,
        }
    }))
}

#[server(
    name = GetRunQuestionResults,
    prefix = "/api",
    endpoint = "get_run_question_results"
)]
pub async fn get_run_question_results(
    run_id: Uuid,
    variant_label: String,
    split: Option<EvaluationResultSplit>,
) -> Result<Option<RunQuestionResultsDto>, ServerFnError> {
    ctx::<Arc<EvaluationServices>>()?
        .evaluation_query_service
        .get_run_question_results(run_id, &variant_label, split)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = GetRecentRuns, prefix = "/api", endpoint = "get_recent_runs")]
pub async fn get_recent_runs(limit: u32) -> Result<Vec<RecentEvaluationRunDto>, ServerFnError> {
    let limit = limit.clamp(1, 100);
    let services = ctx::<Arc<EvaluationServices>>()?;
    let query = &services.evaluation_query_service;
    let ingestion = ctx::<Arc<IngestionServices>>()?;
    let documents = &ingestion.source_document_query_service;

    let runs = query
        .list_recent_runs(limit)
        .await
        .map_err(|e| map_app_error(&e))?;

    let doc_index = build_doc_index(documents).await?;

    Ok(runs.iter().map(|r| map_run_to_dto(r, &doc_index)).collect())
}

#[server(
    name = GetEvaluationRunsPage,
    prefix = "/api",
    endpoint = "get_evaluation_runs_page"
)]
pub async fn get_evaluation_runs_page(
    query: RunListQueryDto,
) -> Result<RunListPageDto, ServerFnError> {
    let cursor = match query.cursor.as_deref() {
        None => None,
        Some(value) => Some(decode_run_cursor(value)?),
    };

    let domain_query = RunListQuery {
        cursor,
        limit: if query.limit == 0 { 25 } else { query.limit },
        statuses: query
            .statuses
            .iter()
            .map(|s| match s {
                RunStatusFilterDto::Completed => RunStatusFilter::Completed,
                RunStatusFilterDto::Running => RunStatusFilter::Running,
                RunStatusFilterDto::Failed => RunStatusFilter::Failed,
                RunStatusFilterDto::Pending => RunStatusFilter::Pending,
            })
            .collect(),
        kinds: query
            .kinds
            .iter()
            .map(|k| match k {
                RunKindDto::Optimization => RunKindFilter::Optimization,
                RunKindDto::Manual => RunKindFilter::Manual,
            })
            .collect(),
    };

    let services = ctx::<Arc<EvaluationServices>>()?;
    let svc = &services.evaluation_query_service;
    let ingestion = ctx::<Arc<IngestionServices>>()?;
    let documents = &ingestion.source_document_query_service;

    let page = svc
        .list_runs_page(domain_query)
        .await
        .map_err(|e| map_app_error(&e))?;

    let doc_index = build_doc_index(documents).await?;

    let items = page
        .items
        .iter()
        .map(|r| map_run_to_dto(r, &doc_index))
        .collect();

    let status_counts = page
        .status_counts
        .into_iter()
        .map(|(filter, count)| RunStatusFacetDto {
            status: status_filter_to_dto(filter),
            count,
        })
        .collect();

    let kind_counts = page
        .kind_counts
        .into_iter()
        .map(|(filter, count)| RunKindFacetDto {
            kind: kind_filter_to_dto(filter),
            count,
        })
        .collect();

    let next_cursor = page.next_cursor.as_ref().map(|c: &RunListCursor| {
        use time::format_description::well_known::Rfc3339;
        let created_at = c
            .created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| String::new());
        format!("{}|{}", created_at, c.run_id)
    });

    Ok(RunListPageDto {
        items,
        next_cursor,
        total_matching: page.total_matching,
        total_all: page.total_all,
        status_counts,
        kind_counts,
    })
}

#[cfg(feature = "ssr")]
fn decode_run_cursor(value: &str) -> Result<RunListCursor, ServerFnError> {
    use crate::server::application::AppError;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    let (created_at, run_id_str) = value
        .rsplit_once('|')
        .ok_or_else(|| map_app_error(&AppError::Validation("invalid run cursor".into())))?;
    let run_id = Uuid::parse_str(run_id_str)
        .map_err(|e| map_app_error(&AppError::Validation(format!("cursor uuid: {e}"))))?;
    let created_at = OffsetDateTime::parse(created_at, &Rfc3339)
        .map_err(|e| map_app_error(&AppError::Validation(format!("cursor timestamp: {e}"))))?;
    Ok(RunListCursor { created_at, run_id })
}

#[cfg(feature = "ssr")]
fn status_filter_to_dto(filter: RunStatusFilter) -> RunStatusFilterDto {
    match filter {
        RunStatusFilter::Completed => RunStatusFilterDto::Completed,
        RunStatusFilter::Running => RunStatusFilterDto::Running,
        RunStatusFilter::Failed => RunStatusFilterDto::Failed,
        RunStatusFilter::Pending => RunStatusFilterDto::Pending,
    }
}

#[cfg(feature = "ssr")]
fn kind_filter_to_dto(filter: RunKindFilter) -> RunKindDto {
    match filter {
        RunKindFilter::Optimization => RunKindDto::Optimization,
        RunKindFilter::Manual => RunKindDto::Manual,
    }
}

#[cfg(feature = "ssr")]
async fn build_doc_index(
    documents: &SourceDocumentQueryService,
) -> Result<HashMap<Uuid, String>, ServerFnError> {
    Ok(documents
        .list()
        .await
        .map_err(|e| map_app_error(&e))?
        .into_iter()
        .map(|d| (d.document_id, d.title))
        .collect())
}

#[cfg(feature = "ssr")]
fn map_run_to_dto(
    run: &EvaluationRunReadModel,
    doc_index: &HashMap<Uuid, String>,
) -> RecentEvaluationRunDto {
    let best = if let Some(snap) = run.best_variant.clone() {
        Some(BestVariantDto {
            label: snap.label,
            config: snap.config.into(),
            options: snap.options.into(),
            score: snap.score,
            metrics: snap.metrics.into(),
        })
    } else {
        let policy = run.scoring_policy;
        run.variant_results
            .iter()
            .max_by(|a, b| {
                policy
                    .score(&a.metrics())
                    .partial_cmp(&policy.score(&b.metrics()))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|v| BestVariantDto {
                label: v.variant_label.clone(),
                config: v.variant_config.into(),
                options: v.options.clone().into(),
                score: policy.score(&v.metrics()),
                metrics: v.metrics().into(),
            })
    };

    let failure_reason = match &run.status {
        EvaluationRunStatus::Failed { reason } if !reason.is_empty() => Some(reason.clone()),
        _ => None,
    };

    let optimization_dto = run.optimization.clone().map(Into::into);
    let kind = RunKindDto::from_optimization(optimization_dto.as_ref());

    RecentEvaluationRunDto {
        run_id: run.run_id,
        dataset_id: run.dataset_id,
        document_id: run.document_id,
        document_title: doc_index.get(&run.document_id).cloned(),
        status: run.status.as_str().to_string(),
        variant_count: run.variants_count,
        variants_scored: run.variants_scored,
        failure_reason,
        created_at: run.created_at.to_string(),
        best,
        kind,
        optimization: optimization_dto,
    }
}
