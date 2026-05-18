use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};
use crate::shared::reference_data::VectorStoreKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    pub index_id: Uuid,
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}

impl CatalogEntry for VectorIndex {
    fn id(&self) -> Uuid {
        self.index_id
    }

    fn natural_key(&self) -> String {
        self.name.clone()
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.name.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "vector index name cannot be empty".into(),
            ));
        }
        if self.dimensions == 0 {
            return Err(CatalogError::ValidationError(
                "vector index dimensions must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}
