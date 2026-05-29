pub mod aggregates;
pub mod catalog;
pub mod chat;
pub mod comprehension;
pub mod connector;
pub mod discovery;
pub mod docs;
pub mod evaluation;
pub mod indexing;
pub mod ingestion;
pub mod platform;
pub mod repositories;
pub mod retrieval;

use std::collections::HashMap;
use std::sync::Arc;

use axum::Extension;
use axum::Router;
use leptos::context::provide_context;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::SourceDocumentCommandHandler;
use crate::server::infrastructure::shared::clients::{CloudflareApi, LlamaServerApi, OllamaApi};
use crate::server::infrastructure::shared::event_sourcing::spawn_postgres_event_listener;
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::shared::id::UuidGenerator;
use crate::server::infrastructure::shared::time::SystemClock;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use crate::server::setup::seed::seed_if_empty;

use self::aggregates::build_aggregate_wirings;
use self::catalog::{CatalogDeps, CatalogServices};
use self::chat::{ChatDeps, ChatServices};
use self::comprehension::{ComprehensionDeps, ComprehensionServices};
use self::connector::{ConnectorDeps, ConnectorServices};
use self::discovery::{DiscoveryDeps, DiscoveryServices};
use self::docs::{DocsDeps, DocsServices};
use self::evaluation::{EvaluationDeps, EvaluationServices};
use self::indexing::{IndexingDeps, IndexingServices};
use self::ingestion::{IngestionDeps, IngestionServices};
use self::platform::{PlatformDeps, PlatformServices};
use self::repositories::build_repositories;
use self::retrieval::{RetrievalDeps, RetrievalServices};

pub struct App {
    pub platform: Arc<PlatformServices>,
    pub catalog: Arc<CatalogServices>,
    pub connector: Arc<ConnectorServices>,
    pub discovery: Arc<DiscoveryServices>,
    pub indexing: Arc<IndexingServices>,
    pub ingestion: Arc<IngestionServices>,
    pub retrieval: Arc<RetrievalServices>,
    pub chat: Arc<ChatServices>,
    pub comprehension: Arc<ComprehensionServices>,
    pub evaluation: Arc<EvaluationServices>,
    pub docs: Arc<DocsServices>,
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
    let llama_server_api = Arc::new(LlamaServerApi::new(
        Arc::clone(&http),
        config.llama_server.base_url.clone(),
    ));

    let repos = build_repositories(&pool, &config, &cf_api)?;
    let wirings = build_aggregate_wirings(&pool);

    let source_document_command_handler = SourceDocumentCommandHandler::new(
        Arc::clone(&wirings.source_document.command_processor),
        Arc::clone(&id_generator),
        Arc::clone(&clock),
    );

    let platform = PlatformServices::build(PlatformDeps {
        config: &config,
        http: Arc::clone(&http),
        cf_api: Arc::clone(&cf_api),
        ollama_api: Arc::clone(&ollama_api),
        llama_server_api: Arc::clone(&llama_server_api),
        repos: &repos,
    })?;

    let mut wakeups: HashMap<String, Vec<Arc<Notify>>> = HashMap::new();

    let catalog = CatalogServices::build(CatalogDeps {
        pool: pool.clone(),
        id_generator: Arc::clone(&id_generator),
        cf_api: Arc::clone(&cf_api),
        repos: &repos,
        wirings: &wirings,
        embedding_service: Arc::clone(&platform.embedding_service),
        generation_service: Arc::clone(&platform.generation_service),
        rerank_service: Arc::clone(&platform.rerank_service),
        event_bus: Arc::clone(&platform.event_bus),
        wakeups: &mut wakeups,
    })?;

    let discovery = DiscoveryServices::build(DiscoveryDeps {
        cf_api: Arc::clone(&cf_api),
        ollama_api: Arc::clone(&ollama_api),
    })?;

    let connector = ConnectorServices::build(ConnectorDeps {
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        http: Arc::clone(&http),
        repos: &repos,
        wirings: &wirings,
        event_bus: Arc::clone(&platform.event_bus),
        wakeups: &mut wakeups,
    })?;

    let indexing = IndexingServices::build(IndexingDeps {
        pool: pool.clone(),
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        repos: &repos,
        wirings: &wirings,
        event_bus: Arc::clone(&platform.event_bus),
        job_registry: Arc::clone(&platform.job_registry),
        activity_registry: Arc::clone(&platform.activity_registry),
        chunker_registry: Arc::clone(&platform.chunker_registry),
        embedding_service: Arc::clone(&platform.embedding_service),
        vector_index_resolver: Arc::clone(&catalog.vector_index_resolver),
        index_profile_resolver: Arc::clone(&catalog.index_profile_resolver),
        kv_store: Arc::clone(&repos.kv_store),
        wakeups: &mut wakeups,
    })?;

    let ingestion = IngestionServices::build(IngestionDeps {
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        http: Arc::clone(&http),
        repos: &repos,
        wirings: &wirings,
        event_bus: Arc::clone(&platform.event_bus),
        markdown_parser: Arc::clone(&platform.markdown_parser),
        index_profile_resolver: Arc::clone(&catalog.index_profile_resolver),
        chunking_configuration_query_service: Arc::clone(
            &catalog.chunking_configuration_query_service,
        ),
        connector_registry: Arc::clone(&connector.connector_registry),
        connector_query_service: Arc::clone(&connector.connector_query_service),
        source_document_command_handler: Arc::clone(&source_document_command_handler),
        indexing_command_handler: Arc::clone(&indexing.indexing_command_handler),
        wakeups: &mut wakeups,
    })?;

    let retrieval = RetrievalServices::build(RetrievalDeps {
        repos: &repos,
        retrieval_profile_resolver: Arc::clone(&catalog.retrieval_profile_resolver),
        embedding_service: Arc::clone(&platform.embedding_service),
        vector_index_resolver: Arc::clone(&catalog.vector_index_resolver),
        rerank_service: Arc::clone(&platform.rerank_service),
    })?;

    let chat = ChatServices::build(ChatDeps {
        repos: &repos,
        retrieval_profile_resolver: Arc::clone(&catalog.retrieval_profile_resolver),
        embedding_service: Arc::clone(&platform.embedding_service),
        vector_index_resolver: Arc::clone(&catalog.vector_index_resolver),
        generation_service: Arc::clone(&platform.generation_service),
        rerank_service: Arc::clone(&platform.rerank_service),
    })?;

    let comprehension = ComprehensionServices::build(ComprehensionDeps {
        pool: pool.clone(),
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        repos: &repos,
        wirings: &wirings,
        event_bus: Arc::clone(&platform.event_bus),
        job_registry: Arc::clone(&platform.job_registry),
        activity_registry: Arc::clone(&platform.activity_registry),
        chunker_registry: Arc::clone(&platform.chunker_registry),
        chunk_set_command_handler: Arc::clone(&indexing.chunk_set_command_handler),
        generation_service: Arc::clone(&platform.generation_service),
        wakeups: &mut wakeups,
    })?;

    let evaluation = EvaluationServices::build(EvaluationDeps {
        pool: pool.clone(),
        clock: Arc::clone(&clock),
        id_generator: Arc::clone(&id_generator),
        repos: &repos,
        wirings: &wirings,
        event_bus: Arc::clone(&platform.event_bus),
        job_registry: Arc::clone(&platform.job_registry),
        activity_registry: Arc::clone(&platform.activity_registry),
        chunker_registry: Arc::clone(&platform.chunker_registry),
        chunk_set_command_handler: Arc::clone(&indexing.chunk_set_command_handler),
        document_map_command_handler: Arc::clone(&comprehension.document_map_command_handler),
        embedding_service: Arc::clone(&platform.embedding_service),
        generation_service: Arc::clone(&platform.generation_service),
        index_profile_resolver: Arc::clone(&catalog.index_profile_resolver),
        source_document_query_service: Arc::clone(&ingestion.source_document_query_service),
        wakeups: &mut wakeups,
    })?;

    let docs = DocsServices::build(DocsDeps {
        guides_dir: config.docs_guides_dir.clone(),
        markdown_parser: Arc::clone(&platform.markdown_parser),
    })?;

    spawn_postgres_event_listener(pool, wakeups);

    let app = App {
        platform: Arc::new(platform),
        catalog: Arc::new(catalog),
        connector: Arc::new(connector),
        discovery: Arc::new(discovery),
        indexing: Arc::new(indexing),
        ingestion: Arc::new(ingestion),
        retrieval: Arc::new(retrieval),
        chat: Arc::new(chat),
        comprehension: Arc::new(comprehension),
        evaluation: Arc::new(evaluation),
        docs: Arc::new(docs),
    };

    app.seed_if_empty().await;
    Ok(app)
}

impl App {
    pub fn provide_contexts(&self) {
        provide_context(Arc::clone(&self.platform));
        provide_context(Arc::clone(&self.catalog));
        provide_context(Arc::clone(&self.connector));
        provide_context(Arc::clone(&self.discovery));
        provide_context(Arc::clone(&self.indexing));
        provide_context(Arc::clone(&self.ingestion));
        provide_context(Arc::clone(&self.retrieval));
        provide_context(Arc::clone(&self.chat));
        provide_context(Arc::clone(&self.comprehension));
        provide_context(Arc::clone(&self.evaluation));
        provide_context(Arc::clone(&self.docs));
    }

    pub fn apply_axum_extensions(&self, router: Router) -> Router {
        router
            .layer(Extension(Arc::clone(&self.platform.event_bus)))
            .layer(Extension(Arc::clone(&self.platform.job_registry)))
            .layer(Extension(Arc::clone(
                &self.ingestion.source_document_ingest_service,
            )))
            .layer(Extension(Arc::clone(&self.chat)))
    }

    async fn seed_if_empty(&self) {
        if let Err(e) = seed_if_empty(
            &self.catalog.chunking_configuration_query_service,
            &self.catalog.sweep_template_query_service,
            &self.catalog.configuration_query_service,
            &self.catalog.index_profile_query_service,
            &self.catalog.retrieval_profile_query_service,
            &self.catalog.embedding_model_command_handler,
            &self.catalog.generation_model_command_handler,
            &self.catalog.reranker_model_command_handler,
            &self.catalog.vector_index_command_handler,
            &self.catalog.chunking_configuration_command_handler,
            &self.catalog.index_profile_command_handler,
            &self.catalog.retrieval_profile_command_handler,
            &self.catalog.sweep_template_command_handler,
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
