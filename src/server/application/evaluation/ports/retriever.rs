use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;

pub struct RetrievalQuery {
    pub embedding_set_id: Uuid,
    pub query_vector: Vec<f32>,
    pub top_k: u32,
    pub min_score: f32,
}

pub struct RetrievedChunk {
    pub chunk_id: Uuid,
    pub score: f32,
}

#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &RetrievalQuery) -> Result<Vec<RetrievedChunk>, AppError>;
}
