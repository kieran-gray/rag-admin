use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::connector::{ConnectorImpl, ConnectorRegistry};
use crate::server::application::connector_sync::{BulkImportService, ConnectorSyncService};
use crate::server::application::evaluation::dataset::EvaluationDatasetEffectExecutor;
use crate::server::application::evaluation::ports::LlmJudge;
use crate::server::application::evaluation::run::{
    EvaluationRunEffectDispatcher, ExecuteVariantEffectExecutor, FinalizeRunEffectExecutor,
    OptimizeRunEffectExecutor, TrialScorer,
};
use crate::server::application::indexing::{IndexingEffect, IndexingEffectExecutor};
use crate::server::application::ports::{Clock, HttpClient, IdGenerator};
use crate::server::application::source_document::ports::{HtmlToMarkdown, PdfToMarkdown};
use crate::server::application::source_document::{
    DocumentNormalizerRegistry, SourceDocumentIngestService, SourceDocumentIngestServiceDeps,
};
use crate::server::application::spawn_activity_projection;
use crate::server::domain::configuration::defaults::{
    make_configuration_defaults_projector, ConfigurationDefaults,
};
use crate::server::domain::configuration::embedding_model::{
    make_embedding_model_projector, EmbeddingModelCatalog,
};
use crate::server::domain::configuration::generation_model::{
    make_generation_model_projector, GenerationModelCatalog,
};
use crate::server::domain::configuration::sweep_template::{SweepTemplate, SweepTemplateProjector};
use crate::server::domain::configuration::vector_index::{
    make_vector_index_projector, VectorIndexCatalog,
};
use crate::server::domain::connector::{Connector, ConnectorProjector};
use crate::server::domain::connector_import::{ConnectorImport, ConnectorImportProjector};
use crate::server::domain::connector_sync::{ConnectorSync, ConnectorSyncProjector};
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::dataset::effects::EvaluationDatasetEffect;
use crate::server::domain::evaluation::dataset::projector::EvaluationDatasetProjector;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::effects::EvaluationRunEffect;
use crate::server::domain::evaluation::run::projector::EvaluationRunProjector;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::projector::IndexingProjector;
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::domain::source_document::projector::SourceDocumentProjector;
use crate::server::infrastructure::connector::SitemapConnector;
use crate::server::infrastructure::evaluation::LlmJudgeAdapter;
use crate::server::infrastructure::shared::event_sourcing::{
    spawn_postgres_event_listener, PostgresJobQueue,
};
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::source_document::normalizers::html::HtmdConverter;
use crate::server::infrastructure::source_document::normalizers::pdf::PdfExtractConverter;
use crate::server::infrastructure::source_document::{
    HtmlNormalizer, MarkdownNormalizer, PdfNormalizer, PlainTextNormalizer,
};
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::compose::services::Services;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;
use event_sourcing::job_queue::JobQueue;
use event_sourcing::process_manager::ProcessManager;

pub struct Workflows {
    pub source_document_ingest_service: Arc<SourceDocumentIngestService>,
    pub connector_registry: Arc<ConnectorRegistry>,
    pub connector_sync_service: Arc<ConnectorSyncService>,
    pub bulk_import_service: Arc<BulkImportService>,
}

pub struct WorkflowsDeps<'a> {
    pub config: &'a Config,
    pub pool: PgPool,
    pub http: Arc<ReqwestHttpClient>,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub event_bus: Arc<EventBus>,
    pub repos: &'a Repositories,
    pub services: &'a Services,
    pub wirings: &'a AggregateWirings,
}

pub fn launch_workflows(deps: WorkflowsDeps<'_>) -> Result<Workflows, SetupError> {
    let WorkflowsDeps {
        config: _,
        pool,
        http,
        clock,
        id_generator,
        event_bus,
        repos,
        services,
        wirings,
    } = deps;

    spawn_activity_projection(
        Arc::clone(&services.activity_registry),
        Arc::clone(&event_bus),
    );

    let mut wakeups: HashMap<String, Arc<Notify>> = HashMap::new();
    let checkpoint = Arc::clone(&repos.checkpoint);

    spawn_driver::<EmbeddingModelCatalog, ()>(
        Arc::clone(&wirings.embedding_model.event_store),
        vec![Arc::new(make_embedding_model_projector(
            Arc::clone(&repos.embedding_model) as _,
        ))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<GenerationModelCatalog, ()>(
        Arc::clone(&wirings.generation_model.event_store),
        vec![Arc::new(make_generation_model_projector(
            Arc::clone(&repos.generation_model) as _,
        ))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<VectorIndexCatalog, ()>(
        Arc::clone(&wirings.vector_index.event_store),
        vec![Arc::new(make_vector_index_projector(
            Arc::clone(&repos.vector_index) as _,
        ))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<SweepTemplate, ()>(
        Arc::clone(&wirings.sweep_template.event_store),
        vec![Arc::new(SweepTemplateProjector::new(Arc::clone(
            &repos.sweep_template,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<ConfigurationDefaults, ()>(
        Arc::clone(&wirings.defaults.event_store),
        vec![Arc::new(make_configuration_defaults_projector(
            Arc::clone(&repos.chunking_configuration),
            Arc::clone(&repos.pipeline_configuration),
            Arc::clone(&repos.sweep_template),
        ))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<Connector, ()>(
        Arc::clone(&wirings.connector.event_store),
        vec![Arc::new(ConnectorProjector::new(Arc::clone(
            &repos.connector,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<ConnectorImport, ()>(
        Arc::clone(&wirings.connector_import.event_store),
        vec![Arc::new(ConnectorImportProjector::new(Arc::clone(
            &repos.connector_import,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<ConnectorSync, ()>(
        Arc::clone(&wirings.connector_sync.event_store),
        vec![Arc::new(ConnectorSyncProjector::new(
            Arc::clone(&repos.connector_sync),
            Arc::clone(&repos.connector_discovered_item),
        ))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<SourceDocument, ()>(
        Arc::clone(&wirings.source_document.event_store),
        vec![Arc::new(SourceDocumentProjector::new(Arc::clone(
            &repos.source_document,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    let indexing_job_queue: Arc<dyn JobQueue<IndexingEffect>> =
        Arc::new(PostgresJobQueue::<IndexingEffect>::new(pool.clone()));

    let indexing_effect_executor = IndexingEffectExecutor::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.indexing),
        Arc::clone(&repos.blob_store),
        Arc::clone(&services.chunking_engine),
        Arc::clone(&repos.chunk_set),
        Arc::clone(&services.embedding_service),
        Arc::clone(&repos.embedding_set),
        Arc::clone(&services.vector_index_resolver),
        Arc::clone(&services.pipeline_resolver),
        Arc::clone(&repos.kv_store),
        Arc::clone(&wirings.indexing.command_processor),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    let indexing_process_manager = Arc::new(ProcessManager::<Indexing, IndexingEffect>::new(
        Arc::clone(&wirings.indexing.aggregate_repository),
        indexing_job_queue,
        indexing_effect_executor,
    ));

    spawn_driver::<Indexing, IndexingEffect>(
        Arc::clone(&wirings.indexing.event_store),
        vec![Arc::new(IndexingProjector::new(Arc::clone(
            &repos.indexing,
        )))],
        Some(indexing_process_manager),
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    let dataset_job_queue: Arc<dyn JobQueue<EvaluationDatasetEffect>> = Arc::new(
        PostgresJobQueue::<EvaluationDatasetEffect>::new(pool.clone()),
    );
    let run_job_queue: Arc<dyn JobQueue<EvaluationRunEffect>> =
        Arc::new(PostgresJobQueue::<EvaluationRunEffect>::new(pool.clone()));

    let dataset_effect_executor = EvaluationDatasetEffectExecutor::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.blob_store),
        Arc::clone(&repos.evaluation_dataset),
        Arc::clone(&services.evaluation_generator),
        Arc::clone(&services.embedding_service),
        Arc::clone(&wirings.dataset.command_processor),
        Arc::clone(&wirings.dataset.aggregate_repository),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
    );

    let trial_scorer = TrialScorer::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.blob_store),
        Arc::clone(&services.chunking_engine),
        Arc::clone(&repos.chunk_set),
        Arc::clone(&services.embedding_service),
        Arc::clone(&repos.embedding_set),
        Arc::clone(&repos.evaluation_dataset),
        Arc::clone(&services.evaluation_retriever),
        Arc::clone(&services.pipeline_resolver),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    let execute_variant_executor = ExecuteVariantEffectExecutor::new(
        Arc::clone(&trial_scorer),
        Arc::clone(&wirings.run.command_processor),
        Arc::clone(&wirings.run.aggregate_repository),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
    );

    let finalize_run_executor = FinalizeRunEffectExecutor::new(
        Arc::clone(&trial_scorer),
        Arc::clone(&wirings.run.command_processor),
        Arc::clone(&repos.evaluation_run),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
    );

    let judge_adapter: Arc<dyn LlmJudge> =
        LlmJudgeAdapter::new(Arc::clone(&services.generation_service));

    let optimize_effect_executor = OptimizeRunEffectExecutor::new(
        Arc::clone(&trial_scorer),
        Arc::clone(&wirings.run.command_processor),
        Arc::clone(&wirings.run.event_store),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
        Some(judge_adapter),
    );

    let run_effect_dispatcher = EvaluationRunEffectDispatcher::new(
        execute_variant_executor,
        finalize_run_executor,
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
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<EvaluationRun, EvaluationRunEffect>(
        Arc::clone(&wirings.run.event_store),
        vec![Arc::new(EvaluationRunProjector::new(Arc::clone(
            &repos.evaluation_run,
        )))],
        Some(run_process_manager),
        checkpoint,
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    spawn_postgres_event_listener(pool, wakeups);

    let html_to_markdown: Arc<dyn HtmlToMarkdown> = HtmdConverter::new();
    let http_port: Arc<dyn HttpClient> = Arc::clone(&http) as Arc<dyn HttpClient>;

    let sitemap_connector: Arc<dyn ConnectorImpl> = SitemapConnector::new(Arc::clone(&http_port));
    let mut connector_registry = ConnectorRegistry::new();
    connector_registry.register(sitemap_connector);
    let connector_registry = Arc::new(connector_registry);

    let pdf_to_markdown: Arc<dyn PdfToMarkdown> = PdfExtractConverter::new();

    let mut normalizer_registry = DocumentNormalizerRegistry::new();
    normalizer_registry.register(MarkdownNormalizer::new());
    normalizer_registry.register(PlainTextNormalizer::new());
    normalizer_registry.register(HtmlNormalizer::new(
        Arc::clone(&html_to_markdown),
        Arc::clone(&clock),
    ));
    normalizer_registry.register(PdfNormalizer::new(pdf_to_markdown));
    let normalizer_registry = Arc::new(normalizer_registry);

    let source_document_ingest_service =
        SourceDocumentIngestService::new(SourceDocumentIngestServiceDeps {
            source_document_command_handler: Arc::clone(&services.source_document_command_handler),
            indexing_command_handler: Arc::clone(&services.indexing_command_handler),
            source_document_repository: Arc::clone(&repos.source_document),
            blob_store: Arc::clone(&repos.blob_store),
            connector_registry: Arc::clone(&connector_registry),
            connector_query_service: Arc::clone(&services.connector_query_service),
            connector_import_command_handler: Arc::clone(
                &services.connector_import_command_handler,
            ),
            pipeline_resolver: Arc::clone(&services.pipeline_resolver),
            http_client: http_port,
            normalizer_registry,
            clock: Arc::clone(&clock),
            id_generator: Arc::clone(&id_generator),
        });

    let connector_sync_service = ConnectorSyncService::new(
        Arc::clone(&services.connector_sync_command_handler),
        Arc::clone(&services.connector_sync_query_service),
        Arc::clone(&connector_registry),
        Arc::clone(&services.connector_query_service),
        Arc::clone(&services.connector_import_command_handler),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    let bulk_import_service = BulkImportService::new(
        Arc::clone(&source_document_ingest_service),
        Arc::clone(&services.pipeline_resolver),
        Arc::clone(&services.chunking_configuration_query_service),
    );

    Ok(Workflows {
        source_document_ingest_service,
        connector_registry,
        connector_sync_service,
        bulk_import_service,
    })
}
