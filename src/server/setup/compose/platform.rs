use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::chunking::chunkers::{
    BertChunker, DarnChunker, LlmChunker, SectionChunker,
};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::embedding::ports::Embedder;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::llm::ports::GenerationClient;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::{MarkdownParser, Tokenizer};
use crate::server::application::{spawn_activity_projection, ActivityRegistry, JobRegistry};
use crate::server::infrastructure::embedding::{OllamaEmbedder, WorkersAiEmbedder};
use crate::server::infrastructure::llm::{OllamaGenerationClient, WorkersAiGenerationClient};
use crate::server::infrastructure::shared::clients::{CloudflareApi, OllamaApi};
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::shared::markdown::MarkdownRsParser;
use crate::server::infrastructure::shared::tokenizer::{TiktokenTokenizer, DEFAULT_TIKTOKEN_MODEL};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;
use crate::shared::reference_data::AiProviderKind;
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
        let embedding_service =
            EmbeddingService::new(embedders, Arc::clone(&repos.embedding_model));

        let ollama_generation_client: Arc<dyn GenerationClient> = OllamaGenerationClient::new(
            Arc::clone(&http),
            config.ollama.base_url.clone(),
            config.ollama.num_ctx,
        );
        let workers_ai_generation_client: Arc<dyn GenerationClient> =
            WorkersAiGenerationClient::new(Arc::clone(&cf_api));
        let generation_clients: HashMap<AiProviderKind, Arc<dyn GenerationClient>> =
            HashMap::from([
                (
                    AiProviderKind::Ollama,
                    Arc::clone(&ollama_generation_client),
                ),
                (
                    AiProviderKind::Cloudflare,
                    Arc::clone(&workers_ai_generation_client),
                ),
            ]);
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
