use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use event_sourcing::error::ProjectionError;

use super::entity::RetrievalProfile;
use super::read_model::RetrievalProfileReadModel;

#[derive(Debug, Error)]
pub enum RetrievalProfileRepositoryError {
    #[error("retrieval profile repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait RetrievalProfileRepository: CatalogRepository<RetrievalProfile> + Send + Sync {
    async fn load_all(
        &self,
    ) -> Result<Vec<RetrievalProfileReadModel>, RetrievalProfileRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<RetrievalProfileReadModel>, RetrievalProfileRepositoryError>;
}

impl From<RetrievalProfileRepositoryError> for ProjectionError {
    fn from(value: RetrievalProfileRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
