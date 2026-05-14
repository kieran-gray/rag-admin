use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::chunking::chunkers::{
    register_builtin_chunkers, BuiltinChunkerDeps,
};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::ports::EvaluationDefaultsStore;
use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, ChunkingConfigurationService, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    PipelineConfigurationQueryService, PipelineConfigurationService, PipelineResolver,
    SweepTemplateCommandHandler, SweepTemplateQueryService, VectorIndexCatalogCommandHandler,
};
use crate::server::application::embedding::ports::Embedder;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::ports::{EvaluationGenerator, Retriever};
use crate::server::application::evaluation::query_service::EvaluationQueryService;
use crate::server::application::indexing::IndexingCommandHandler;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::ChatService;
use crate::server::application::ports::{ChatClient, Clock, IdGenerator, MarkdownParser};
use crate::server::application::query::QueryService;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::application::source_document::{
    SourceDocumentCommandHandler, SourceDocumentQueryService,
};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::domain::configuration::kinds::{AiProviderKind, VectorStoreKind};
use crate::server::infrastructure::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::configuration::FileEvaluationDefaultsStore;
use crate::server::infrastructure::embedding::{OllamaEmbedder, WorkersAiEmbedder};
use crate::server::infrastructure::evaluation::{ChatBasedEvaluationGenerator, PgvectorRetriever};
use crate::server::infrastructure::http_client::ReqwestHttpClient;
use crate::server::infrastructure::llm::OllamaChatClient;
use crate::server::infrastructure::markdown::MarkdownRsParser;
use crate::server::infrastructure::tokenizer::HuggingFaceTokenizer;
use crate::server::infrastructure::vector::{
    CloudflareVectorIndexProvider, PostgresVectorIndexProvider,
};
use crate::server::setup::compose::event_sourcing::AggregateWirings;
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use crate::server::setup::paths::{evaluation_defaults_path, tokenizer_path};

use sqlx::PgPool;

pub struct Services {
    pub embedding_model_command_handler: Arc<EmbeddingModelCatalogCommandHandler>,
    pub generation_model_command_handler: Arc<GenerationModelCatalogCommandHandler>,
    pub vector_index_command_handler: Arc<VectorIndexCatalogCommandHandler>,
    pub sweep_template_command_handler: Arc<SweepTemplateCommandHandler>,
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,

    pub pipeline_configuration_service: Arc<PipelineConfigurationService>,
    pub chunking_configuration_service: Arc<ChunkingConfigurationService>,

    pub configuration_query_service: Arc<ConfigurationQueryService>,
    pub pipeline_configuration_query_service: Arc<PipelineConfigurationQueryService>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub sweep_template_query_service: Arc<SweepTemplateQueryService>,
    pub evaluation_query_service: Arc<EvaluationQueryService>,
    pub source_document_query_service: Arc<SourceDocumentQueryService>,
    pub query_service: Arc<QueryService>,

    pub embedding_service: Arc<EmbeddingService>,
    pub chat_service: Arc<ChatService>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub pipeline_resolver: Arc<PipelineResolver>,

    pub evaluation_defaults_store: Arc<dyn EvaluationDefaultsStore>,
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
        clock: _,
        id_generator,
        http,
        cf_api,
        ollama_api,
        repos,
        wirings,
    } = deps;

    let tokenizer = HuggingFaceTokenizer::load_or_fetch(tokenizer_path(), Arc::clone(&http))
        .await
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

    let ollama_chat_client: Arc<dyn ChatClient> =
        OllamaChatClient::new(Arc::clone(&http), config.ollama.base_url.clone());
    let chat_clients: HashMap<AiProviderKind, Arc<dyn ChatClient>> = HashMap::from([(
        AiProviderKind::Ollama,
        Arc::clone(&ollama_chat_client) as Arc<dyn ChatClient>,
    )]);
    let chat_service = ChatService::new(chat_clients, Arc::clone(&repos.generation_model));

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
        Arc::clone(&embedding_service),
        Arc::clone(&chat_service),
        Arc::clone(&vector_index_resolver),
    );

    let evaluation_defaults_store: Arc<dyn EvaluationDefaultsStore> =
        FileEvaluationDefaultsStore::new(evaluation_defaults_path());

    let evaluation_generator: Arc<dyn EvaluationGenerator> =
        ChatBasedEvaluationGenerator::new(Arc::clone(&chat_service));
    let evaluation_retriever: Arc<dyn Retriever> = Arc::new(PgvectorRetriever::new(pool));

    let chunking_engine = build_chunking_engine(
        tokenizer,
        Arc::clone(&markdown_parser),
        Arc::clone(&ollama_chat_client),
        Arc::clone(&repos.generation_model),
    );

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
    let sweep_template_command_handler = SweepTemplateCommandHandler::new(
        Arc::clone(&wirings.sweep_template.command_processor),
        Arc::clone(&id_generator),
    );
    let source_document_command_handler =
        SourceDocumentCommandHandler::new(Arc::clone(&wirings.source_document.command_processor));
    let indexing_command_handler =
        IndexingCommandHandler::new(Arc::clone(&wirings.indexing.command_processor));

    let pipeline_configuration_service = PipelineConfigurationService::new(
        Arc::clone(&repos.pipeline_configuration),
        Arc::clone(&repos.embedding_model),
        Arc::clone(&repos.vector_index),
    );
    let chunking_configuration_service =
        ChunkingConfigurationService::new(Arc::clone(&repos.chunking_configuration));

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
    );
    let chunking_configuration_query_service =
        ChunkingConfigurationQueryService::new(Arc::clone(&repos.chunking_configuration));
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
    );
    let query_service = QueryService::new(
        Arc::clone(&pipeline_resolver),
        Arc::clone(&embedding_service),
        Arc::clone(&vector_index_resolver),
        Arc::clone(&repos.source_document),
    );

    Ok(Services {
        embedding_model_command_handler,
        generation_model_command_handler,
        vector_index_command_handler,
        sweep_template_command_handler,
        source_document_command_handler,
        indexing_command_handler,
        pipeline_configuration_service,
        chunking_configuration_service,
        configuration_query_service,
        pipeline_configuration_query_service,
        chunking_configuration_query_service,
        sweep_template_query_service,
        evaluation_query_service,
        source_document_query_service,
        query_service,
        embedding_service,
        chat_service,
        vector_index_resolver,
        pipeline_resolver,
        evaluation_defaults_store,
        evaluation_generator,
        evaluation_retriever,
        chunking_engine,
        markdown_parser,
        job_registry,
        activity_registry,
    })
}

fn build_chunking_engine(
    tokenizer: Arc<HuggingFaceTokenizer>,
    markdown_parser: Arc<dyn MarkdownParser>,
    chat_client: Arc<dyn ChatClient>,
    generation_models: Arc<
        dyn crate::server::domain::configuration::generation_model::GenerationModelRepository,
    >,
) -> Arc<ChunkerRegistry> {
    let mut chunking_engine = ChunkerRegistry::new(tokenizer, markdown_parser);
    register_builtin_chunkers(
        &mut chunking_engine,
        BuiltinChunkerDeps {
            chat_client,
            generation_models,
        },
    );
    Arc::new(chunking_engine)
}
