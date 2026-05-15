use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}
