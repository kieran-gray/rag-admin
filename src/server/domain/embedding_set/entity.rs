use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::embedding_model::EmbeddingModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSet {
    pub embedding_set_id: Uuid,
    pub chunk_set_id: Uuid,
    pub embedding_model_id: Uuid,
    pub embedding_model_snapshot: EmbeddingModel,
    pub dimensions: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkEmbedding {
    pub chunk_id: Uuid,
    pub embedding_set_id: Uuid,
    pub vector: Vec<f32>,
}
