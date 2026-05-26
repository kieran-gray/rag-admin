use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::chunk_set::ChunkSetCommandHandler;
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::IndexProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::dataset::EvaluationDatasetEffectExecutor;
use crate::server::application::evaluation::ports::{EvaluationGenerator, LlmJudge, Retriever};
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::evaluation::run::{
    CompleteRunEffectExecutor, EvaluationRunEffectDispatcher, ExecuteVariantEffectExecutor,
    OptimizeRunEffectExecutor, TrialScorer,
};
use crate::server::application::evaluation::{
    EvaluationDatasetCommandHandler, EvaluationRunCommandHandler,
};
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::SourceDocumentQueryService;
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::domain::chunk_set::ref_projectors::EvaluationRunChunkSetRefProjector;
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::dataset::effects::EvaluationDatasetEffect;
use crate::server::domain::evaluation::dataset::projector::EvaluationDatasetProjector;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::effects::EvaluationRunEffect;
use crate::server::domain::evaluation::run::projector::EvaluationRunProjector;
use crate::server::infrastructure::evaluation::{
    LlmEvaluationGenerator, LlmJudgeAdapter, PgvectorRetriever,
};
use crate::server::infrastructure::shared::event_sourcing::PostgresJobQueue;
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;
use event_sourcing::job_queue::JobQueue;
use event_sourcing::process_manager::ProcessManager;

pub struct EvaluationServices {
    pub evaluation_dataset_command_handler: Arc<EvaluationDatasetCommandHandler>,
    pub evaluation_run_command_handler: Arc<EvaluationRunCommandHandler>,
    pub evaluation_query_service: Arc<EvaluationQueryService>,
}

pub struct EvaluationDeps<'a> {
    pub pool: PgPool,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub event_bus: Arc<EventBus>,
    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
    pub chunker_registry: Arc<ChunkerRegistry>,
    pub chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
    pub embedding_service: Arc<EmbeddingService>,
    pub generation_service: Arc<GenerationService>,
    pub index_profile_resolver: Arc<IndexProfileResolver>,
    pub source_document_query_service: Arc<SourceDocumentQueryService>,
    pub wakeups: &'a mut HashMap<String, Arc<Notify>>,
}

impl EvaluationServices {
    pub fn build(deps: EvaluationDeps<'_>) -> Result<Self, SetupError> {
        let EvaluationDeps {
            pool,
            clock,
            id_generator,
            repos,
            wirings,
            event_bus,
            job_registry,
            activity_registry,
            chunker_registry,
            chunk_set_command_handler,
            embedding_service,
            generation_service,
            index_profile_resolver,
            source_document_query_service,
            wakeups,
        } = deps;

        let evaluation_query_service = EvaluationQueryService::new(
            Arc::clone(&repos.evaluation_dataset),
            Arc::clone(&repos.evaluation_run),
        );

        let evaluation_generator: Arc<dyn EvaluationGenerator> =
            LlmEvaluationGenerator::new(Arc::clone(&generation_service));
        let evaluation_retriever: Arc<dyn Retriever> =
            Arc::new(PgvectorRetriever::new(pool.clone()));

        let evaluation_dataset_command_handler = EvaluationDatasetCommandHandler::new(
            Arc::clone(&wirings.dataset.command_processor),
            Arc::clone(&embedding_service),
            Arc::clone(&generation_service),
            source_document_query_service,
            Arc::clone(&clock),
            Arc::clone(&id_generator),
        );
        let evaluation_run_command_handler = EvaluationRunCommandHandler::new(
            Arc::clone(&wirings.run.command_processor),
            Arc::clone(&evaluation_query_service),
            Arc::clone(&repos.source_document),
            Arc::clone(&index_profile_resolver),
            Arc::clone(&repos.chunking_configuration),
            Arc::clone(&clock),
            Arc::clone(&id_generator),
        );

        let dataset_job_queue: Arc<dyn JobQueue<EvaluationDatasetEffect>> = Arc::new(
            PostgresJobQueue::<EvaluationDatasetEffect>::new(pool.clone()),
        );
        let run_job_queue: Arc<dyn JobQueue<EvaluationRunEffect>> =
            Arc::new(PostgresJobQueue::<EvaluationRunEffect>::new(pool));

        let dataset_effect_executor = EvaluationDatasetEffectExecutor::new(
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.blob_store),
            Arc::clone(&repos.evaluation_dataset),
            evaluation_generator,
            Arc::clone(&embedding_service),
            Arc::clone(&wirings.dataset.command_processor),
            Arc::clone(&wirings.dataset.aggregate_repository),
            Arc::clone(&job_registry),
            Arc::clone(&activity_registry),
            Arc::clone(&clock),
        );

        let trial_scorer = TrialScorer::new(
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.blob_store),
            chunker_registry,
            Arc::clone(&repos.chunk_set),
            chunk_set_command_handler,
            embedding_service,
            Arc::clone(&repos.embedding_set),
            Arc::clone(&repos.evaluation_dataset),
            evaluation_retriever,
            index_profile_resolver,
            Arc::clone(&generation_service),
            Arc::clone(&clock),
            Arc::clone(&id_generator),
        );

        let execute_variant_executor = ExecuteVariantEffectExecutor::new(
            Arc::clone(&trial_scorer),
            Arc::clone(&wirings.run.command_processor),
            Arc::clone(&wirings.run.aggregate_repository),
            Arc::clone(&job_registry),
            Arc::clone(&activity_registry),
            Arc::clone(&clock),
        );

        let complete_run_executor = CompleteRunEffectExecutor::new(
            Arc::clone(&wirings.run.command_processor),
            Arc::clone(&repos.evaluation_run),
            Arc::clone(&job_registry),
            Arc::clone(&activity_registry),
            Arc::clone(&clock),
        );

        let judge_adapter: Arc<dyn LlmJudge> = LlmJudgeAdapter::new(generation_service);

        let optimize_effect_executor = OptimizeRunEffectExecutor::new(
            Arc::clone(&trial_scorer),
            Arc::clone(&wirings.run.command_processor),
            Arc::clone(&wirings.run.event_store),
            Arc::clone(&repos.chunking_configuration),
            job_registry,
            activity_registry,
            clock,
            Some(judge_adapter),
        );

        let run_effect_dispatcher = EvaluationRunEffectDispatcher::new(
            execute_variant_executor,
            complete_run_executor,
            optimize_effect_executor,
        );

        let dataset_process_manager = Arc::new(ProcessManager::new(
            Arc::clone(&wirings.dataset.aggregate_repository),
            dataset_job_queue,
            dataset_effect_executor,
        ));
        let run_process_manager = Arc::new(ProcessManager::new(
            Arc::clone(&wirings.run.aggregate_repository),
            run_job_queue,
            run_effect_dispatcher,
        ));

        spawn_driver::<EvaluationDataset, EvaluationDatasetEffect>(
            Arc::clone(&wirings.dataset.event_store),
            vec![Arc::new(EvaluationDatasetProjector::new(Arc::clone(
                &repos.evaluation_dataset,
            )))],
            Some(dataset_process_manager),
            Arc::clone(&repos.checkpoint),
            Arc::clone(&event_bus),
            wakeups,
        );
        spawn_driver::<EvaluationRun, EvaluationRunEffect>(
            Arc::clone(&wirings.run.event_store),
            vec![
                Arc::new(EvaluationRunProjector::new(
                    Arc::clone(&repos.evaluation_run),
                    Arc::clone(&repos.evaluation_dataset),
                )),
                Arc::new(EvaluationRunChunkSetRefProjector::new(Arc::clone(
                    &repos.chunk_set,
                ))),
            ],
            Some(run_process_manager),
            Arc::clone(&repos.checkpoint),
            event_bus,
            wakeups,
        );

        Ok(Self {
            evaluation_dataset_command_handler,
            evaluation_run_command_handler,
            evaluation_query_service,
        })
    }
}
