use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProfile {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

impl CatalogEntry for IndexProfile {
    fn id(&self) -> Uuid {
        self.index_profile_id
    }

    fn natural_key(&self) -> String {
        self.name.clone()
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.name.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "index profile name cannot be empty".into(),
            ));
        }
        if self.dimensions == 0 {
            return Err(CatalogError::ValidationError(
                "index profile dimensions must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}
