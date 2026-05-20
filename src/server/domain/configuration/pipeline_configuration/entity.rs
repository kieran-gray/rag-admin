use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfiguration {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub generation_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

impl CatalogEntry for PipelineConfiguration {
    fn id(&self) -> Uuid {
        self.pipeline_configuration_id
    }

    fn natural_key(&self) -> String {
        self.name.clone()
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.name.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "pipeline configuration name cannot be empty".into(),
            ));
        }
        if self.dimensions == 0 {
            return Err(CatalogError::ValidationError(
                "pipeline configuration dimensions must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}
