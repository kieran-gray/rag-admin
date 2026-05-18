use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};
use crate::shared::reference_data::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

impl CatalogEntry for EmbeddingModel {
    fn id(&self) -> Uuid {
        self.embedding_model_id
    }

    fn natural_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.model)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.model.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "embedding model cannot be empty".into(),
            ));
        }
        if self.dimensions == 0 {
            return Err(CatalogError::ValidationError(
                "embedding dimensions must be greater than zero".into(),
            ));
        }
        if !self.kind.model_id_well_formed(&self.model) {
            return Err(CatalogError::ValidationError(format!(
                "model id '{}' is not well-formed for provider kind {}",
                self.model,
                self.kind.as_str()
            )));
        }
        Ok(())
    }
}
