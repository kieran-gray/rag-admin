use std::sync::Arc;

use crate::server::application::chunking::chunkers::{
    BertChunker, DarnChunker, LlmChunker, SectionChunker,
};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::embedding::{EmbedderRegistry, EmbeddingService};
use crate::server::application::llm::ports::GenerationClient;
use crate::server::application::llm::{GenerationClientRegistry, GenerationService};
use crate::server::application::ports::{MarkdownParser, Tokenizer};
use crate::server::application::{spawn_activity_projection, ActivityRegistry, JobRegistry};
use crate::server::infrastructure::embedding::{
    cloudflare as cf_embedder, ollama as ollama_embedder,
};
use crate::server::infrastructure::llm::{
    ollama_generation_client as ollama_llm, workers_ai_generation_client as workers_ai_llm,
};
use crate::server::infrastructure::shared::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::shared::markdown::MarkdownRsParser;
use crate::server::infrastructure::shared::tokenizer::{TiktokenTokenizer, DEFAULT_TIKTOKEN_MODEL};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;

pub struct PlatformServices {
    pub event_bus: Arc<EventBus>,
    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
    pub tokenizer: Arc<dyn Tokenizer>,
    pub markdown_parser: Arc<dyn MarkdownParser>,
    pub chunker_registry: Arc<ChunkerRegistry>,
    pub embedding_service: Arc<EmbeddingService>,
    pub generation_service: Arc<GenerationService>,
    pub ollama_generation_client: Arc<dyn GenerationClient>,
}

pub struct PlatformDeps<'a> {
    pub config: &'a Config,
    pub http: Arc<ReqwestHttpClient>,
    pub cf_api: Arc<CloudflareApi>,
    pub ollama_api: Arc<OllamaApi>,
    pub repos: &'a Repositories,
}

impl PlatformServices {
    pub fn build(deps: PlatformDeps<'_>) -> Result<Self, SetupError> {
        let PlatformDeps {
            config,
            http,
            cf_api,
            ollama_api,
            repos,
        } = deps;

        let event_bus = Arc::new(EventBus::new());
        let job_registry = Arc::new(JobRegistry::new());
        let activity_registry = Arc::new(ActivityRegistry::new());

        spawn_activity_projection(Arc::clone(&activity_registry), Arc::clone(&event_bus));

        let tokenizer: Arc<dyn Tokenizer> = TiktokenTokenizer::for_model(DEFAULT_TIKTOKEN_MODEL)
            .map_err(|e| SetupError::Internal(format!("tokenizer: {e}")))?;
        let markdown_parser: Arc<dyn MarkdownParser> = Arc::new(MarkdownRsParser);

        let mut embedders = EmbedderRegistry::new();
        cf_embedder::register(&mut embedders, Arc::clone(&cf_api));
        ollama_embedder::register(&mut embedders, Arc::clone(&ollama_api));
        let embedding_service =
            EmbeddingService::new(embedders, Arc::clone(&repos.embedding_model));

        let mut generation_clients = GenerationClientRegistry::new();
        let ollama_generation_client = ollama_llm::register(
            &mut generation_clients,
            Arc::clone(&http),
            config.ollama.base_url.clone(),
            config.ollama.num_ctx,
        );
        workers_ai_llm::register(&mut generation_clients, Arc::clone(&cf_api));
        let generation_service =
            GenerationService::new(generation_clients, Arc::clone(&repos.generation_model));

        let mut chunker_registry =
            ChunkerRegistry::new(Arc::clone(&tokenizer), Arc::clone(&markdown_parser));
        chunker_registry.add(Arc::new(SectionChunker {}));
        chunker_registry.add(Arc::new(BertChunker {}));
        chunker_registry.add(Arc::new(DarnChunker {}));
        chunker_registry.add(Arc::new(LlmChunker::create(
            Arc::clone(&ollama_generation_client),
            Arc::clone(&repos.generation_model),
        )));
        let chunker_registry = Arc::new(chunker_registry);

        Ok(Self {
            event_bus,
            job_registry,
            activity_registry,
            tokenizer,
            markdown_parser,
            chunker_registry,
            embedding_service,
            generation_service,
            ollama_generation_client,
        })
    }
}
