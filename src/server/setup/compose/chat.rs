use std::sync::Arc;

use crate::server::application::chat::ChatService;
use crate::server::application::configuration::RetrievalProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::GenerationService;
use crate::server::application::rerank::RerankService;
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;

pub struct ChatServices {
    pub chat_service: Arc<ChatService>,
}

pub struct ChatDeps<'a> {
    pub repos: &'a Repositories,
    pub retrieval_profile_resolver: Arc<RetrievalProfileResolver>,
    pub embedding_service: Arc<EmbeddingService>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub generation_service: Arc<GenerationService>,
    pub rerank_service: Arc<RerankService>,
}

impl ChatServices {
    pub fn build(deps: ChatDeps<'_>) -> Result<Self, SetupError> {
        let chat_service = ChatService::new(
            deps.retrieval_profile_resolver,
            deps.embedding_service,
            deps.vector_index_resolver,
            deps.generation_service,
            deps.rerank_service,
            Arc::clone(&deps.repos.source_document),
        );
        Ok(Self { chat_service })
    }
}
