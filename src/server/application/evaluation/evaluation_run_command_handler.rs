use std::sync::Arc;

use uuid::Uuid;

use crate::contracts::{EvaluationJobInfo, RunEvaluationRequestDto, RunOptimizationRequestDto};
use crate::core::{ChunkingVariant, EvaluationRunOptions, OptimizationConfig};
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{EvaluationRunCommand, RequestRun};
use crate::server::domain::evaluation::run::scoring_policy::ScoringPolicy;
use crate::server::event_sourcing::CommandProcessor;

pub struct EvaluationRunCommandHandler {
    processor: Arc<CommandProcessor<EvaluationRun>>,
    queries: Arc<EvaluationQueryService>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl EvaluationRunCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<EvaluationRun>>,
        queries: Arc<EvaluationQueryService>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            queries,
            clock,
            id_generator,
        })
    }

    pub async fn start_optimization(
        &self,
        request: RunOptimizationRequestDto,
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

        let run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id,
                    dataset_id: request.dataset_id,
                    pipeline_configuration_id: request.pipeline_configuration_id,
                    document_id: dataset.document_id,
                    document_version: dataset.document_version,
                    variants: Vec::new(),
                    options: Vec::new(),
                    autotune_request: None,
                    optimization: Some(request.optimization),
                    scoring_policy: ScoringPolicy::default(),
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

        let run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id,
                    dataset_id: request.dataset_id,
                    pipeline_configuration_id: request.pipeline_configuration_id,
                    document_id: dataset.document_id,
                    document_version: dataset.document_version,
                    variants: request.variants,
                    options: request.options,
                    autotune_request: request.autotune,
                    optimization: None,
                    scoring_policy: ScoringPolicy::default(),
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
            budget: original.budget,
            scope: original.scope,
            judges_enabled: original.judges_enabled,
            seed: None,
        };

        let new_run_id = self.id_generator.new_uuid();
        self.processor
            .handle(
                new_run_id,
                EvaluationRunCommand::RequestRun(RequestRun {
                    run_id: new_run_id,
                    dataset_id: run.dataset_id,
                    pipeline_configuration_id: run.pipeline_configuration_id,
                    document_id: run.document_id,
                    document_version: run.document_version,
                    variants: Vec::<ChunkingVariant>::new(),
                    options: Vec::<EvaluationRunOptions>::new(),
                    autotune_request: None,
                    optimization: Some(optimization),
                    scoring_policy: ScoringPolicy::default(),
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        Ok(new_run_id)
    }
}
