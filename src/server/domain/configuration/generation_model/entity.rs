use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}
