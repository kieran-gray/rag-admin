use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};
use crate::shared::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfiguration {
    pub chunking_configuration_id: Uuid,
    pub name: String,
    pub config: ChunkingConfig,
}

impl CatalogEntry for ChunkingConfiguration {
    fn id(&self) -> Uuid {
        self.chunking_configuration_id
    }

    fn natural_key(&self) -> String {
        self.name.clone()
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.name.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "chunking configuration name cannot be empty".into(),
            ));
        }
        Ok(())
    }
}
