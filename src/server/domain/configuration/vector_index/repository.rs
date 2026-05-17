use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;

use super::entity::VectorIndex;
use crate::server::event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum VectorIndexRepositoryError {
    #[error("vector index repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait VectorIndexRepository: CatalogRepository<VectorIndex> {
    async fn load_all(&self) -> Result<Vec<VectorIndex>, VectorIndexRepositoryError>;

    async fn find_by_id(
        &self,
        index_id: Uuid,
    ) -> Result<Option<VectorIndex>, VectorIndexRepositoryError>;
}

impl From<VectorIndexRepositoryError> for ProjectionError {
    fn from(value: VectorIndexRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
