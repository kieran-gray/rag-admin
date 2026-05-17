use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteChunkingEffect {
    pub indexing_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteEmbeddingEffect {
    pub indexing_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteIndexingEffect {
    pub indexing_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IndexingEffect {
    ExecuteChunking(ExecuteChunkingEffect),
    ExecuteEmbedding(ExecuteEmbeddingEffect),
    ExecuteIndexing(ExecuteIndexingEffect),
}
