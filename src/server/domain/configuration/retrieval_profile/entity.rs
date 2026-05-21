use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalProfile {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile_id: Uuid,
    pub generation_model_id: Uuid,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

impl CatalogEntry for RetrievalProfile {
    fn id(&self) -> Uuid {
        self.retrieval_profile_id
    }

    fn natural_key(&self) -> String {
        self.name.clone()
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.name.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "retrieval profile name cannot be empty".into(),
            ));
        }
        if self.default_top_k == 0 {
            return Err(CatalogError::ValidationError(
                "retrieval profile default_top_k must be greater than zero".into(),
            ));
        }
        if self.default_min_score_milli > 1000 {
            return Err(CatalogError::ValidationError(
                "retrieval profile default_min_score_milli must be between 0 and 1000".into(),
            ));
        }
        Ok(())
    }
}
