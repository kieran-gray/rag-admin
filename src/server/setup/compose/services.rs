use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::chat::ChatService;
use crate::server::application::chunking::chunkers::{
    BertChunker, DarnChunker, LlmChunker, SectionChunker,
};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::{
    ChunkingConfigurationCatalogCommandHandler, ChunkingConfigurationQueryService,
    ConfigurationDefaultsCommandHandler, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    PipelineConfigurationCatalogCommandHandler, PipelineConfigurationQueryService,
    PipelineResolver, SweepTemplateCommandHandler, SweepTemplateQueryService,
    VectorIndexCatalogCommandHandler,
};
use crate::server::application::connector::{ConnectorCommandHandler, ConnectorQueryService};
use crate::server::application::connector_import::{
    ConnectorImportCommandHandler, ConnectorImportQueryService,
};
use crate::server::application::connector_sync::{
    ConnectorSyncCommandHandler, ConnectorSyncQueryService,
};
use crate::server::application::embedding::ports::Embedder;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::ports::{EvaluationGenerator, Retriever};
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::evaluation::{
    EvaluationDatasetCommandHandler, EvaluationRunCommandHandler,
};
use crate::server::application::indexing::IndexingCommandHandler;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::ports::GenerationClient;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::Tokenizer;
use crate::server::application::ports::{Clock, IdGenerator, MarkdownParser};
use crate::server::application::retrieval::RetrievalService;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::application::source_document::{
    SourceDocumentCommandHandler, SourceDocumentQueryService,
};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::infrastructure::embedding::{OllamaEmbedder, WorkersAiEmbedder};
use crate::server::infrastructure::evaluation::{LlmEvaluationGenerator, PgvectorRetriever};
use crate::server::infrastructure::llm::{OllamaGenerationClient, WorkersAiGenerationClient};
use crate::server::infrastructure::shared::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::shared::markdown::MarkdownRsParser;
use crate::server::infrastructure::shared::tokenizer::{TiktokenTokenizer, DEFAULT_TIKTOKEN_MODEL};
use crate::server::infrastructure::vector::{
    CloudflareVectorIndexProvider, PostgresVectorIndexProvider,
};
use crate::server::setup::compose::aggregates::AggregateWirings;
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};

use sqlx::PgPool;

pub struct Services {
    pub embedding_model_command_handler: Arc<EmbeddingModelCatalogCommandHandler>,
    pub generation_model_command_handler: Arc<GenerationModelCatalogCommandHandler>,
    pub vector_index_command_handler: Arc<VectorIndexCatalogCommandHandler>,
    pub configuration_defaults_command_handler: Arc<ConfigurationDefaultsCommandHandler>,
    pub sweep_template_command_handler: Arc<SweepTemplateCommandHandler>,
    pub connector_command_handler: Arc<ConnectorCommandHandler>,
    pub connector_query_service: Arc<ConnectorQueryService>,
    pub connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
    pub connector_import_query_service: Arc<ConnectorImportQueryService>,
    pub connector_sync_command_handler: Arc<ConnectorSyncCommandHandler>,
    pub connector_sync_query_service: Arc<ConnectorSyncQueryService>,
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub evaluation_dataset_command_handler: Arc<EvaluationDatasetCommandHandler>,
    pub evaluation_run_command_handler: Arc<EvaluationRunCommandHandler>,

    pub pipeline_configuration_command_handler: Arc<PipelineConfigurationCatalogCommandHandler>,
    pub chunking_configuration_command_handler: Arc<ChunkingConfigurationCatalogCommandHandler>,

    pub configuration_query_service: Arc<ConfigurationQueryService>,
    pub pipeline_configuration_query_service: Arc<PipelineConfigurationQueryService>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub sweep_template_query_service: Arc<SweepTemplateQueryService>,
    pub evaluation_query_service: Arc<EvaluationQueryService>,
    pub source_document_query_service: Arc<SourceDocumentQueryService>,
    pub retrieval_service: Arc<RetrievalService>,
    pub chat_service: Arc<ChatService>,

    pub embedding_service: Arc<EmbeddingService>,
    pub generation_service: Arc<GenerationService>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub pipeline_resolver: Arc<PipelineResolver>,

    pub evaluation_generator: Arc<dyn EvaluationGenerator>,
    pub evaluation_retriever: Arc<dyn Retriever>,
    pub chunking_engine: Arc<ChunkerRegistry>,
    pub markdown_parser: Arc<dyn MarkdownParser>,

    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
}

pub struct ServicesDeps<'a> {
    pub config: &'a Config,
    pub pool: PgPool,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub http: Arc<ReqwestHttpClient>,
    pub cf_api: Arc<CloudflareApi>,
    pub ollama_api: Arc<OllamaApi>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
}

pub async fn build_services(deps: ServicesDeps<'_>) -> Result<Services, SetupError> {
    let ServicesDeps {
        config,
        pool,
        clock,
        id_generator,
        http,
        cf_api,
        ollama_api,
        repos,
        wirings,
    } = deps;

    let tokenizer: Arc<dyn Tokenizer> = TiktokenTokenizer::for_model(DEFAULT_TIKTOKEN_MODEL)
        .map_err(|e| SetupError::Internal(format!("tokenizer: {e}")))?;
    let markdown_parser: Arc<dyn MarkdownParser> = Arc::new(MarkdownRsParser);

    let embedders: HashMap<AiProviderKind, Arc<dyn Embedder>> = HashMap::from([
        (
            AiProviderKind::Cloudflare,
            WorkersAiEmbedder::new(Arc::clone(&cf_api)) as Arc<dyn Embedder>,
        ),
        (
            AiProviderKind::Ollama,
            OllamaEmbedder::new(Arc::clone(&ollama_api)) as Arc<dyn Embedder>,
        ),
    ]);
    let embedding_service = EmbeddingService::new(embedders, Arc::clone(&repos.embedding_model));

    let ollama_generation_client: Arc<dyn GenerationClient> = OllamaGenerationClient::new(
        Arc::clone(&http),
        config.ollama.base_url.clone(),
        config.ollama.num_ctx,
    );
    let workers_ai_generation_client: Arc<dyn GenerationClient> =
        WorkersAiGenerationClient::new(Arc::clone(&cf_api));
    let generation_clients: HashMap<AiProviderKind, Arc<dyn GenerationClient>> = HashMap::from([
        (
            AiProviderKind::Ollama,
            Arc::clone(&ollama_generation_client) as Arc<dyn GenerationClient>,
        ),
        (
            AiProviderKind::Cloudflare,
            Arc::clone(&workers_ai_generation_client) as Arc<dyn GenerationClient>,
        ),
    ]);
    let generation_service =
        GenerationService::new(generation_clients, Arc::clone(&repos.generation_model));

    let vector_providers: HashMap<VectorStoreKind, Arc<dyn VectorIndexProvider>> = HashMap::from([
        (
            VectorStoreKind::CloudflareVectorize,
            CloudflareVectorIndexProvider::new(Arc::clone(&cf_api)) as Arc<dyn VectorIndexProvider>,
        ),
        (
            VectorStoreKind::Postgres,
            PostgresVectorIndexProvider::new(pool.clone()) as Arc<dyn VectorIndexProvider>,
        ),
    ]);
    let vector_index_resolver =
        VectorIndexResolver::new(vector_providers, Arc::clone(&repos.vector_index));

    let pipeline_resolver = PipelineResolver::new(
        Arc::clone(&repos.pipeline_configuration),
        Arc::clone(&repos.connector),
        Arc::clone(&repos.configuration_defaults),
        Arc::clone(&embedding_service),
        Arc::clone(&generation_service),
        Arc::clone(&vector_index_resolver),
    );

    let evaluation_generator: Arc<dyn EvaluationGenerator> =
        LlmEvaluationGenerator::new(Arc::clone(&generation_service));
    let evaluation_retriever: Arc<dyn Retriever> = Arc::new(PgvectorRetriever::new(pool));

    let mut chunker_registry = ChunkerRegistry::new(tokenizer, Arc::clone(&markdown_parser));
    chunker_registry.add(Arc::new(SectionChunker {}));
    chunker_registry.add(Arc::new(BertChunker {}));
    chunker_registry.add(Arc::new(DarnChunker {}));
    chunker_registry.add(Arc::new(LlmChunker::create(
        Arc::clone(&ollama_generation_client),
        Arc::clone(&repos.generation_model),
    )));
    let chunking_engine = Arc::new(chunker_registry);

    let job_registry = Arc::new(JobRegistry::new());
    let activity_registry = Arc::new(ActivityRegistry::new());

    let embedding_model_command_handler = EmbeddingModelCatalogCommandHandler::new(Arc::clone(
        &wirings.embedding_model.command_processor,
    ));
    let generation_model_command_handler = GenerationModelCatalogCommandHandler::new(Arc::clone(
        &wirings.generation_model.command_processor,
    ));
    let vector_index_command_handler =
        VectorIndexCatalogCommandHandler::new(Arc::clone(&wirings.vector_index.command_processor));
    let configuration_defaults_command_handler =
        ConfigurationDefaultsCommandHandler::new(Arc::clone(&wirings.defaults.command_processor));
    let sweep_template_command_handler = SweepTemplateCommandHandler::new(
        Arc::clone(&wirings.sweep_template.command_processor),
        Arc::clone(&configuration_defaults_command_handler),
        Arc::clone(&id_generator),
    );
    let connector_query_service = ConnectorQueryService::new(Arc::clone(&repos.connector));
    let connector_command_handler = ConnectorCommandHandler::new(
        Arc::clone(&wirings.connector.command_processor),
        Arc::clone(&connector_query_service),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );
    let connector_import_command_handler = ConnectorImportCommandHandler::new(
        Arc::clone(&wirings.connector_import.command_processor),
        Arc::clone(&clock),
    );
    let connector_import_query_service =
        ConnectorImportQueryService::new(Arc::clone(&repos.connector_import));
    let connector_sync_command_handler = ConnectorSyncCommandHandler::new(
        Arc::clone(&wirings.connector_sync.command_processor),
        Arc::clone(&clock),
    );
    let connector_sync_query_service = ConnectorSyncQueryService::new(
        Arc::clone(&repos.connector_sync),
        Arc::clone(&repos.connector_discovered_item),
        Arc::clone(&repos.connector_import),
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.indexing),
    );
    let source_document_command_handler = SourceDocumentCommandHandler::new(
        Arc::clone(&wirings.source_document.command_processor),
        Arc::clone(&id_generator),
        Arc::clone(&clock),
    );
    let indexing_command_handler = IndexingCommandHandler::new(
        Arc::clone(&wirings.indexing.command_processor),
        Arc::clone(&id_generator),
        Arc::clone(&clock),
    );

    let pipeline_configuration_command_handler = PipelineConfigurationCatalogCommandHandler::new(
        Arc::clone(&wirings.pipeline_configuration.command_processor),
        Arc::clone(&repos.embedding_model),
        Arc::clone(&repos.generation_model),
        Arc::clone(&repos.vector_index),
        Arc::clone(&configuration_defaults_command_handler),
    );
    let chunking_configuration_command_handler = ChunkingConfigurationCatalogCommandHandler::new(
        Arc::clone(&wirings.chunking_configuration.command_processor),
        Arc::clone(&configuration_defaults_command_handler),
    );

    let configuration_query_service = ConfigurationQueryService::new(
        Arc::clone(&repos.embedding_model),
        Arc::clone(&repos.generation_model),
        Arc::clone(&repos.vector_index),
    );
    let pipeline_configuration_query_service = PipelineConfigurationQueryService::new(
        Arc::clone(&repos.pipeline_configuration),
        Arc::clone(&repos.embedding_model),
        Arc::clone(&repos.generation_model),
        Arc::clone(&repos.vector_index),
        Arc::clone(&repos.configuration_defaults),
    );
    let chunking_configuration_query_service = ChunkingConfigurationQueryService::new(
        Arc::clone(&repos.chunking_configuration),
        Arc::clone(&repos.connector),
        Arc::clone(&repos.configuration_defaults),
    );
    let sweep_template_query_service =
        SweepTemplateQueryService::new(Arc::clone(&repos.sweep_template));
    let evaluation_query_service = EvaluationQueryService::new(
        Arc::clone(&repos.evaluation_dataset),
        Arc::clone(&repos.evaluation_run),
    );

    let source_document_query_service = SourceDocumentQueryService::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.indexing),
        Arc::clone(&repos.chunk_set),
        Arc::clone(&repos.blob_store),
        Arc::clone(&markdown_parser),
        Arc::clone(&connector_query_service),
        Arc::clone(&connector_import_query_service),
    );
    let retrieval_service = RetrievalService::new(
        Arc::clone(&pipeline_resolver),
        Arc::clone(&embedding_service),
        Arc::clone(&vector_index_resolver),
        Arc::clone(&repos.source_document),
    );
    let chat_service = ChatService::new(
        Arc::clone(&pipeline_resolver),
        Arc::clone(&embedding_service),
        Arc::clone(&vector_index_resolver),
        Arc::clone(&generation_service),
        Arc::clone(&repos.source_document),
    );

    let evaluation_dataset_command_handler = EvaluationDatasetCommandHandler::new(
        Arc::clone(&wirings.dataset.command_processor),
        Arc::clone(&pipeline_resolver),
        Arc::clone(&source_document_query_service),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );
    let evaluation_run_command_handler = EvaluationRunCommandHandler::new(
        Arc::clone(&wirings.run.command_processor),
        Arc::clone(&evaluation_query_service),
        Arc::clone(&repos.source_document),
        Arc::clone(&pipeline_resolver),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    Ok(Services {
        embedding_model_command_handler,
        generation_model_command_handler,
        vector_index_command_handler,
        configuration_defaults_command_handler,
        sweep_template_command_handler,
        connector_command_handler,
        connector_query_service,
        connector_import_command_handler,
        connector_import_query_service,
        connector_sync_command_handler,
        connector_sync_query_service,
        source_document_command_handler,
        indexing_command_handler,
        evaluation_dataset_command_handler,
        evaluation_run_command_handler,
        pipeline_configuration_command_handler,
        chunking_configuration_command_handler,
        configuration_query_service,
        pipeline_configuration_query_service,
        chunking_configuration_query_service,
        sweep_template_query_service,
        evaluation_query_service,
        source_document_query_service,
        retrieval_service,
        chat_service,
        embedding_service,
        generation_service,
        vector_index_resolver,
        pipeline_resolver,
        evaluation_generator,
        evaluation_retriever,
        chunking_engine,
        markdown_parser,
        job_registry,
        activity_registry,
    })
}
