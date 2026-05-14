pub mod event_sourcing;
pub mod repositories;
pub mod services;
pub mod workflows;

use std::sync::Arc;

use axum::Extension;
use axum::Router;
use leptos::context::provide_context;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::server::application::configuration::ports::EvaluationDefaultsStore;
use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, ChunkingConfigurationService, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    PipelineConfigurationQueryService, PipelineConfigurationService, PipelineResolver,
    SweepTemplateCommandHandler, SweepTemplateQueryService, VectorIndexCatalogCommandHandler,
};
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::ChatService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::query::QueryService;
use crate::server::application::source_document::ports::SourceAdapterRegistry;
use crate::server::application::source_document::{
    SourceDocumentIngestService, SourceDocumentQueryService,
};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::event_bus::EventBus;
use crate::server::infrastructure::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::http_client::ReqwestHttpClient;
use crate::server::infrastructure::id::UuidGenerator;
use crate::server::infrastructure::time::SystemClock;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use crate::server::setup::seed::seed_if_empty;

use self::event_sourcing::build_aggregate_wirings;
use self::repositories::build_repositories;
use self::services::{build_services, ServicesDeps};
use self::workflows::{launch_workflows, WorkflowsDeps};

pub struct App {
    pub embedding_model_command_handler: Arc<EmbeddingModelCatalogCommandHandler>,
    pub generation_model_command_handler: Arc<GenerationModelCatalogCommandHandler>,
    pub vector_index_command_handler: Arc<VectorIndexCatalogCommandHandler>,
    pub pipeline_configuration_service: Arc<PipelineConfigurationService>,
    pub chunking_configuration_service: Arc<ChunkingConfigurationService>,
    pub sweep_template_command_handler: Arc<SweepTemplateCommandHandler>,
    pub configuration_query_service: Arc<ConfigurationQueryService>,
    pub pipeline_configuration_query_service: Arc<PipelineConfigurationQueryService>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub sweep_template_query_service: Arc<SweepTemplateQueryService>,
    pub evaluation_dataset_command_processor: Arc<CommandProcessor<EvaluationDataset>>,
    pub evaluation_run_command_processor: Arc<CommandProcessor<EvaluationRun>>,
    pub evaluation_query_service: Arc<EvaluationQueryService>,
    pub embedding_service: Arc<EmbeddingService>,
    pub chat_service: Arc<ChatService>,
    pub pipeline_resolver: Arc<PipelineResolver>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub evaluation_defaults_store: Arc<dyn EvaluationDefaultsStore>,
    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
    pub source_document_ingest_service: Arc<SourceDocumentIngestService>,
    pub source_document_query_service: Arc<SourceDocumentQueryService>,
    pub query_service: Arc<QueryService>,
    pub source_adapter_registry: Arc<SourceAdapterRegistry>,
    pub event_bus: Arc<EventBus>,
}

pub async fn bootstrap() -> Result<App, SetupError> {
    let config = Config::from_env()?;
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let id_generator: Arc<dyn IdGenerator> = Arc::new(UuidGenerator);
    let pool = connect_database(&config.database_url).await?;

    let http = Arc::new(
        ReqwestHttpClient::new().map_err(|e| SetupError::Internal(format!("http client: {e}")))?,
    );
    let cf_api = Arc::new(CloudflareApi::new(
        Arc::clone(&http),
        config.cloudflare.clone(),
    ));
    let ollama_api = Arc::new(OllamaApi::new(
        Arc::clone(&http),
        config.ollama.base_url.clone(),
    ));

    let event_bus = Arc::new(EventBus::new());

    let repos = build_repositories(&pool, &config, &cf_api)?;
    let wirings = build_aggregate_wirings(&pool);

    let services = build_services(ServicesDeps {
        config: &config,
        pool: pool.clone(),
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        http: Arc::clone(&http),
        cf_api: Arc::clone(&cf_api),
        ollama_api: Arc::clone(&ollama_api),
        repos: &repos,
        wirings: &wirings,
    })
    .await?;

    let workflows = launch_workflows(WorkflowsDeps {
        config: &config,
        pool,
        http,
        clock,
        id_generator,
        event_bus: Arc::clone(&event_bus),
        repos: &repos,
        services: &services,
        wirings: &wirings,
    })?;

    let app = App {
        embedding_model_command_handler: services.embedding_model_command_handler,
        generation_model_command_handler: services.generation_model_command_handler,
        vector_index_command_handler: services.vector_index_command_handler,
        pipeline_configuration_service: services.pipeline_configuration_service,
        chunking_configuration_service: services.chunking_configuration_service,
        sweep_template_command_handler: services.sweep_template_command_handler,
        configuration_query_service: services.configuration_query_service,
        pipeline_configuration_query_service: services.pipeline_configuration_query_service,
        chunking_configuration_query_service: services.chunking_configuration_query_service,
        sweep_template_query_service: services.sweep_template_query_service,
        evaluation_dataset_command_processor: Arc::clone(&wirings.dataset.command_processor),
        evaluation_run_command_processor: Arc::clone(&wirings.run.command_processor),
        evaluation_query_service: services.evaluation_query_service,
        embedding_service: services.embedding_service,
        chat_service: services.chat_service,
        pipeline_resolver: services.pipeline_resolver,
        vector_index_resolver: services.vector_index_resolver,
        evaluation_defaults_store: services.evaluation_defaults_store,
        job_registry: services.job_registry,
        activity_registry: services.activity_registry,
        source_document_ingest_service: workflows.source_document_ingest_service,
        source_document_query_service: services.source_document_query_service,
        query_service: services.query_service,
        source_adapter_registry: workflows.source_adapter_registry,
        event_bus,
    };

    app.seed_if_empty().await;
    Ok(app)
}

impl App {
    pub fn provide_contexts(&self) {
        provide_context(Arc::clone(&self.embedding_model_command_handler));
        provide_context(Arc::clone(&self.generation_model_command_handler));
        provide_context(Arc::clone(&self.vector_index_command_handler));
        provide_context(Arc::clone(&self.pipeline_configuration_service));
        provide_context(Arc::clone(&self.chunking_configuration_service));
        provide_context(Arc::clone(&self.sweep_template_command_handler));
        provide_context(Arc::clone(&self.configuration_query_service));
        provide_context(Arc::clone(&self.pipeline_configuration_query_service));
        provide_context(Arc::clone(&self.chunking_configuration_query_service));
        provide_context(Arc::clone(&self.sweep_template_query_service));
        provide_context(Arc::clone(&self.evaluation_dataset_command_processor));
        provide_context(Arc::clone(&self.evaluation_run_command_processor));
        provide_context(Arc::clone(&self.evaluation_query_service));
        provide_context(Arc::clone(&self.embedding_service));
        provide_context(Arc::clone(&self.chat_service));
        provide_context(Arc::clone(&self.pipeline_resolver));
        provide_context(Arc::clone(&self.vector_index_resolver));
        provide_context(Arc::clone(&self.evaluation_defaults_store));
        provide_context(Arc::clone(&self.job_registry));
        provide_context(Arc::clone(&self.activity_registry));
        provide_context(Arc::clone(&self.source_document_ingest_service));
        provide_context(Arc::clone(&self.source_document_query_service));
        provide_context(Arc::clone(&self.query_service));
        provide_context(Arc::clone(&self.source_adapter_registry));
    }

    pub fn apply_axum_extensions(&self, router: Router) -> Router {
        router
            .layer(Extension(Arc::clone(&self.event_bus)))
            .layer(Extension(Arc::clone(&self.job_registry)))
    }

    async fn seed_if_empty(&self) {
        if let Err(e) = seed_if_empty(
            &self.chunking_configuration_query_service,
            &self.sweep_template_query_service,
            &self.configuration_query_service,
            &self.embedding_model_command_handler,
            &self.generation_model_command_handler,
            &self.vector_index_command_handler,
            &self.chunking_configuration_service,
            &self.sweep_template_command_handler,
        )
        .await
        {
            tracing::warn!("configuration seed: {e}");
        }
    }
}

async fn connect_database(url: &str) -> Result<PgPool, SetupError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .map_err(|e| SetupError::Internal(format!("postgres pool: {e}")))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| SetupError::Internal(format!("migrations: {e}")))?;
    Ok(pool)
}
