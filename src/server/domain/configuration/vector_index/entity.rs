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

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str, dimensions: u32) -> VectorIndex {
        VectorIndex {
            index_id: Uuid::new_v4(),
            kind: VectorStoreKind::Postgres,
            name: name.into(),
            dimensions,
        }
    }

    #[test]
    fn id_returns_index_id() {
        let i = index("docs", 768);
        assert_eq!(i.id(), i.index_id);
    }

    #[test]
    fn natural_key_is_name() {
        let i = index("docs", 768);
        assert_eq!(i.natural_key(), "docs");
    }

    #[test]
    fn validate_accepts_well_formed_index() {
        assert!(index("docs", 1024).validate().is_ok());
    }

    #[test]
    fn validate_rejects_whitespace_only_name() {
        let err = index("   ", 768).validate().unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn validate_rejects_empty_name() {
        let err = index("", 768).validate().unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn validate_rejects_zero_dimensions() {
        let err = index("docs", 0).validate().unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }
}
