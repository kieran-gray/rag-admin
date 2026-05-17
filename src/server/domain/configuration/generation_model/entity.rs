use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::AiProviderKind;
use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

impl CatalogEntry for GenerationModel {
    fn id(&self) -> Uuid {
        self.generation_model_id
    }

    fn natural_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.model)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.model.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "generation model cannot be empty".into(),
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
