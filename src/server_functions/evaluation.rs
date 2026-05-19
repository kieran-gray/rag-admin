use leptos::prelude::*;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::shared::contracts::BestVariantDto;
use crate::shared::contracts::{
    EvaluationDatasetDto, EvaluationDatasetSummaryDto, EvaluationJobInfo, EvaluationRunDto,
    EvaluationRunSummaryDto, RecentEvaluationRunDto, RunEvaluationRequestDto, RunListPageDto,
    RunListQueryDto, RunOptimizationRequestDto,
};
#[cfg(feature = "ssr")]
use crate::shared::contracts::{RunStatusFacetDto, RunStatusFilterDto};

#[cfg(feature = "ssr")]
use crate::server::application::evaluation::query_service::EvaluationQueryService;
#[cfg(feature = "ssr")]
use crate::server::application::evaluation::{
    EvaluationDatasetCommandHandler, EvaluationRunCommandHandler, StartDatasetGenerationRequest,
};
#[cfg(feature = "ssr")]
use crate::server::application::source_document::SourceDocumentQueryService;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::aggregate::EvaluationRunStatus;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::read_model::EvaluationRunReadModel;
#[cfg(feature = "ssr")]
use crate::server::domain::evaluation::run::repository::{
    RunListCursor, RunListQuery, RunStatusFilter,
};
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
    let datasets = ctx::<Arc<EvaluationQueryService>>()?
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

#[server(name = GetDataset, prefix = "/api", endpoint = "get_dataset")]
pub async fn get_dataset(dataset_id: Uuid) -> Result<Option<EvaluationDatasetDto>, ServerFnError> {
    let query = ctx::<Arc<EvaluationQueryService>>()?;

    let dataset = query
        .get_dataset(dataset_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    if let Some(d) = dataset {
        let questions = query
            .load_questions(dataset_id)
            .await
            .map_err(|e| map_app_error(&e))?;

        Ok(Some(EvaluationDatasetDto {
            dataset_id: d.dataset_id,
            document_id: d.document_id,
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
    pipeline_configuration_id: Uuid,
    label: String,
    question_count: u32,
    excerpt_similarity_threshold_milli: u32,
    duplicate_similarity_threshold_milli: u32,
) -> Result<EvaluationJobInfo, ServerFnError> {
    ctx::<Arc<EvaluationDatasetCommandHandler>>()?
        .start_generation(StartDatasetGenerationRequest {
            document_id,
            pipeline_configuration_id,
            label,
            question_count,
            excerpt_similarity_threshold_milli,
            duplicate_similarity_threshold_milli,
        })
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = RenameDataset, prefix = "/api", endpoint = "rename_dataset")]
pub async fn rename_dataset(dataset_id: Uuid, label: String) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationDatasetCommandHandler>>()?
        .rename(dataset_id, label)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = DeleteDataset, prefix = "/api", endpoint = "delete_dataset")]
pub async fn delete_dataset(dataset_id: Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationDatasetCommandHandler>>()?
        .delete(dataset_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = CancelDatasetGeneration, prefix = "/api", endpoint = "cancel_dataset_generation")]
pub async fn cancel_dataset_generation(dataset_id: Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<EvaluationDatasetCommandHandler>>()?
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
    ctx::<Arc<EvaluationRunCommandHandler>>()?
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
    ctx::<Arc<EvaluationRunCommandHandler>>()?
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
    let runs = ctx::<Arc<EvaluationQueryService>>()?
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
    ctx::<Arc<EvaluationRunCommandHandler>>()?
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
    use crate::server::application::configuration::ChunkingConfigurationService;
    use crate::shared::contracts::{
        ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    };

    let trimmed_label = variant_label.trim();
    if trimmed_label.is_empty() {
        return Err(ServerFnError::new("variant_label is required".to_string()));
    }

    let query = ctx::<Arc<EvaluationQueryService>>()?;
    let chunking_service = ctx::<Arc<ChunkingConfigurationService>>()?;

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
            config: chosen.variant_config,
            is_default: false,
        },
    );
    chunking_service
        .handle_dto(cmd)
        .await
        .map_err(|e| map_app_error(&e))?;

    Ok(run_id)
}

#[server(name = GetRun, prefix = "/api", endpoint = "get_run")]
pub async fn get_run(run_id: Uuid) -> Result<Option<EvaluationRunDto>, ServerFnError> {
    let run = ctx::<Arc<EvaluationQueryService>>()?
        .get_run(run_id)
        .await
        .map_err(|e| map_app_error(&e))?;

    Ok(run.map(|r| EvaluationRunDto {
        run_id: r.run_id,
        dataset_id: r.dataset_id,
        status: r.status.as_str().to_string(),
        variants: r.variant_results.into_iter().map(Into::into).collect(),
        created_at: r.created_at.to_string(),
    }))
}

#[server(name = GetRecentRuns, prefix = "/api", endpoint = "get_recent_runs")]
pub async fn get_recent_runs(limit: u32) -> Result<Vec<RecentEvaluationRunDto>, ServerFnError> {
    let limit = limit.clamp(1, 100);
    let query = ctx::<Arc<EvaluationQueryService>>()?;
    let documents = ctx::<Arc<SourceDocumentQueryService>>()?;

    let runs = query
        .list_recent_runs(limit)
        .await
        .map_err(|e| map_app_error(&e))?;

    let doc_index = build_doc_index(&documents).await?;

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
    };

    let svc = ctx::<Arc<EvaluationQueryService>>()?;
    let documents = ctx::<Arc<SourceDocumentQueryService>>()?;

    let page = svc
        .list_runs_page(domain_query)
        .await
        .map_err(|e| map_app_error(&e))?;

    let doc_index = build_doc_index(&documents).await?;

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

    let next_cursor = page
        .next_cursor
        .as_ref()
        .map(|c: &RunListCursor| format!("{}|{}", c.created_at, c.run_id));

    Ok(RunListPageDto {
        items,
        next_cursor,
        total_matching: page.total_matching,
        total_all: page.total_all,
        status_counts,
    })
}

#[cfg(feature = "ssr")]
fn decode_run_cursor(value: &str) -> Result<RunListCursor, ServerFnError> {
    use crate::server::application::AppError;

    let (created_at, run_id_str) = value
        .rsplit_once('|')
        .ok_or_else(|| map_app_error(&AppError::Validation("invalid run cursor".into())))?;
    let run_id = Uuid::parse_str(run_id_str)
        .map_err(|e| map_app_error(&AppError::Validation(format!("cursor uuid: {e}"))))?;
    Ok(RunListCursor {
        created_at: created_at.to_string(),
        run_id,
    })
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
    let policy = run.scoring_policy;
    let best = run
        .variant_results
        .iter()
        .max_by(|a, b| {
            policy
                .score(&a.metrics())
                .partial_cmp(&policy.score(&b.metrics()))
                .unwrap_or(Ordering::Equal)
        })
        .map(|v| BestVariantDto {
            label: v.variant_label.clone(),
            config: v.variant_config,
            options: v.options.clone(),
            score: policy.score(&v.metrics()),
            metrics: v.metrics(),
        });

    let failure_reason = match &run.status {
        EvaluationRunStatus::Failed { reason } if !reason.is_empty() => Some(reason.clone()),
        _ => None,
    };

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
    }
}
