use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::server::application::configuration::IndexProfileResolver;
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationRepository;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{EvaluationRunCommand, RequestRun};
use crate::server::domain::evaluation::run::scoring_policy::ScoringPolicy;
use crate::server::domain::evaluation::value_objects::RunFingerprint;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::shared::contracts::{
    EvaluationJobInfo, RunEvaluationRequestDto, RunOptimizationRequestDto,
};
use crate::shared::{OptimizationConfig, OptimizationScope};
use event_sourcing::CommandProcessor;

pub struct EvaluationRunCommandHandler {
    processor: Arc<CommandProcessor<EvaluationRun>>,
    queries: Arc<EvaluationQueryService>,
    source_documents: Arc<dyn SourceDocumentRepository>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    chunking_configurations: Arc<dyn ChunkingConfigurationRepository>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl EvaluationRunCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<EvaluationRun>>,
        queries: Arc<EvaluationQueryService>,
        source_documents: Arc<dyn SourceDocumentRepository>,
        index_profile_resolver: Arc<IndexProfileResolver>,
        chunking_configurations: Arc<dyn ChunkingConfigurationRepository>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            queries,
            source_documents,
            index_profile_resolver,
            chunking_configurations,
            clock,
            id_generator,
        })
    }

    async fn validate_optimization(&self, config: &OptimizationConfig) -> Result<(), AppError> {
        let Some(id) = config.fixed_chunking_configuration_id else {
            return Ok(());
        };
        if config.scope != OptimizationScope::Retrieval {
            return Err(AppError::Validation(
                "fixed_chunking_configuration_id is only valid when scope = retrieval".into(),
            ));
        }
        self.chunking_configurations
            .find_by_id(id)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!("chunking configuration {id} not found"))
            })?;
        Ok(())
    }

    async fn build_fingerprint(
        &self,
        dataset_content_hash: &str,
        document_id: Uuid,
        document_version: u32,
        index_profile_id: Uuid,
    ) -> Result<RunFingerprint, AppError> {
        let document = self
            .source_documents
            .load(document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {document_id} not found")))?;

        if document.latest_version_number != document_version {
            return Err(AppError::Validation(format!(
                "dataset targets document version {document_version} but current version is {}; \
                 regenerate the dataset before running",
                document.latest_version_number
            )));
        }
        let document_content_hash = document.latest_content_hash.as_hex().to_string();
        if document_content_hash != dataset_content_hash {
            return Err(AppError::Validation(
                "dataset content_hash does not match the current document version hash; \
                 regenerate the dataset before running"
                    .to_string(),
            ));
        }

        let index_profile = self
            .index_profile_resolver
            .resolve(index_profile_id)
            .await?;
        let embedding_model_snapshot = json!({
            "embedding_model_id": index_profile.embedding_model.embedding_model_id,
            "kind": index_profile.embedding_model.kind.as_str(),
            "model": index_profile.embedding_model.model,
            "dimensions": index_profile.embedding_model.dimensions,
        });

        Ok(RunFingerprint {
            document_content_hash,
            dataset_content_hash: dataset_content_hash.to_string(),
            embedding_model_snapshot,
        })
    }

    pub async fn start_optimization(
        &self,
        request: RunOptimizationRequestDto,
    ) -> Result<EvaluationJobInfo, AppError> {
        self.validate_optimization(&request.optimization).await?;

        let dataset = self
            .queries
            .get_dataset(request.dataset_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "evaluation dataset {} not found",
                    request.dataset_id
                ))
            })?;

        let fingerprint = self
            .build_fingerprint(
                &dataset.content_hash,
                dataset.document_id,
                dataset.document_version,
                request.index_profile_id,
            )
            .await?;

        let run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id,
                    dataset_id: request.dataset_id,
                    index_profile_id: request.index_profile_id,
                    retrieval_profile_id: request.retrieval_profile_id,
                    document_id: dataset.document_id,
                    document_version: dataset.document_version,
                    variants: Vec::new(),
                    options: Vec::new(),
                    optimization: Some(request.optimization.into()),
                    scoring_policy: ScoringPolicy::default(),
                    fingerprint,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        Ok(EvaluationJobInfo {
            job_id: run_id.to_string(),
            stream_url: format!("/api/events/ws?stream_id={run_id}"),
        })
    }

    pub async fn start_run(
        &self,
        request: RunEvaluationRequestDto,
    ) -> Result<EvaluationJobInfo, AppError> {
        let dataset = self
            .queries
            .get_dataset(request.dataset_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "evaluation dataset {} not found",
                    request.dataset_id
                ))
            })?;

        let fingerprint = self
            .build_fingerprint(
                &dataset.content_hash,
                dataset.document_id,
                dataset.document_version,
                request.index_profile_id,
            )
            .await?;

        let run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id,
                    dataset_id: request.dataset_id,
                    index_profile_id: request.index_profile_id,
                    retrieval_profile_id: request.retrieval_profile_id,
                    document_id: dataset.document_id,
                    document_version: dataset.document_version,
                    variants: request.variants.into_iter().map(Into::into).collect(),
                    options: request.options.into_iter().map(Into::into).collect(),
                    optimization: None,
                    scoring_policy: ScoringPolicy::default(),
                    fingerprint,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        Ok(EvaluationJobInfo {
            job_id: run_id.to_string(),
            stream_url: format!("/api/events/ws?stream_id={run_id}"),
        })
    }

    pub async fn replicate_optimization(&self, run_id: Uuid) -> Result<Uuid, AppError> {
        let run = self
            .queries
            .get_run(run_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("run {run_id} not found")))?;

        let original = run
            .optimization
            .clone()
            .ok_or_else(|| AppError::Validation("run was not an optimization run".to_string()))?;
        let optimization = OptimizationConfig {
            budget: original.budget.into(),
            scope: original.scope.into(),
            judges_enabled: original.judges_enabled,
            seed: None,
            fixed_chunking_configuration_id: original.fixed_chunking_configuration_id,
        };
        self.validate_optimization(&optimization).await?;

        let dataset = self
            .queries
            .get_dataset(run.dataset_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("evaluation dataset {} not found", run.dataset_id))
            })?;

        let fingerprint = self
            .build_fingerprint(
                &dataset.content_hash,
                run.document_id,
                run.document_version,
                run.index_profile_id,
            )
            .await?;

        let new_run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                new_run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id: new_run_id,
                    dataset_id: run.dataset_id,
                    index_profile_id: run.index_profile_id,
                    retrieval_profile_id: run.retrieval_profile_id,
                    document_id: run.document_id,
                    document_version: run.document_version,
                    variants: Vec::new(),
                    options: Vec::new(),
                    optimization: Some(optimization.into()),
                    scoring_policy: ScoringPolicy::default(),
                    fingerprint,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        Ok(new_run_id)
    }
}
