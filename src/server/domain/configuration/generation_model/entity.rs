use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}
