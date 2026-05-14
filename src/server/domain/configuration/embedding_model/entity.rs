use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}
