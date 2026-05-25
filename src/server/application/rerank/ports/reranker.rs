use async_trait::async_trait;

use crate::server::application::AppError;

#[derive(Debug, Clone)]
pub struct RerankRequest {
    pub model: String,
    pub query: String,
    pub documents: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RerankScore {
    pub index: usize,
    pub score: f32,
}

#[async_trait]
pub trait Reranker: Send + Sync {
    async fn rerank(&self, request: RerankRequest) -> Result<Vec<RerankScore>, AppError>;
}
