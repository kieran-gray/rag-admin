use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::{AiProviderKind, VectorStoreKind};
use crate::server::application::chunking::chunkers::{
    register_builtin_chunkers, BuiltinChunkerDeps,
};
use crate::server::application::chunking::ChunkerRegistry;
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
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::Tokenizer;
use crate::server::application::ports::{Clock, GenerationClient, IdGenerator, MarkdownParser};
use crate::server::application::query::QueryService;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::application::source_document::{
    SourceDocumentCommandHandler, SourceDocumentQueryService,
};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::infrastructure::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::embedding::{OllamaEmbedder, WorkersAiEmbedder};
use crate::server::infrastructure::evaluation::{LlmEvaluationGenerator, PgvectorRetriever};
use crate::server::infrastructure::http_client::ReqwestHttpClient;
use crate::server::infrastructure::llm::{OllamaGenerationClient, WorkersAiGenerationClient};
use crate::server::infrastructure::markdown::MarkdownRsParser;
use crate::server::infrastructure::tokenizer::{TiktokenTokenizer, DEFAULT_TIKTOKEN_MODEL};
use crate::server::infrastructure::vector::{
    CloudflareVectorIndexProvider, PostgresVectorIndexProvider,
};
use crate::server::setup::compose::event_sourcing::AggregateWirings;
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;

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
        clock: _,
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
        Arc::clone(&embedding_service),
        Arc::clone(&generation_service),
        Arc::clone(&vector_index_resolver),
    );

    let evaluation_generator: Arc<dyn EvaluationGenerator> =
        LlmEvaluationGenerator::new(Arc::clone(&generation_service));
    let evaluation_retriever: Arc<dyn Retriever> = Arc::new(PgvectorRetriever::new(pool));

    let chunking_engine = build_chunking_engine(
        tokenizer,
        Arc::clone(&markdown_parser),
        Arc::clone(&ollama_generation_client),
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

fn build_chunking_engine(
    tokenizer: Arc<dyn Tokenizer>,
    markdown_parser: Arc<dyn MarkdownParser>,
    generation_client: Arc<dyn GenerationClient>,
    generation_models: Arc<
        dyn crate::server::domain::configuration::generation_model::GenerationModelRepository,
    >,
) -> Arc<ChunkerRegistry> {
    let mut chunking_engine = ChunkerRegistry::new(tokenizer, markdown_parser);
    register_builtin_chunkers(
        &mut chunking_engine,
        BuiltinChunkerDeps {
            generation_client,
            generation_models,
        },
    );
    Arc::new(chunking_engine)
}
