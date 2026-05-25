use std::sync::Arc;

use crate::server::application::configuration::RetrievalProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::rerank::RerankService;
use crate::server::application::retrieval::RetrievalService;
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;

pub struct RetrievalServices {
    pub retrieval_service: Arc<RetrievalService>,
}

pub struct RetrievalDeps<'a> {
    pub repos: &'a Repositories,
    pub retrieval_profile_resolver: Arc<RetrievalProfileResolver>,
    pub embedding_service: Arc<EmbeddingService>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub rerank_service: Arc<RerankService>,
}

impl RetrievalServices {
    pub fn build(deps: RetrievalDeps<'_>) -> Result<Self, SetupError> {
        let retrieval_service = RetrievalService::new(
            deps.retrieval_profile_resolver,
            deps.embedding_service,
            deps.vector_index_resolver,
            deps.rerank_service,
            Arc::clone(&deps.repos.source_document),
        );

        Ok(Self { retrieval_service })
    }
}
