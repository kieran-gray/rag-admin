use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use event_sourcing::error::ProjectionError;

use super::entity::IndexProfile;
use super::read_model::IndexProfileReadModel;

#[derive(Debug, Error)]
pub enum IndexProfileRepositoryError {
    #[error("index profile repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait IndexProfileRepository: CatalogRepository<IndexProfile> + Send + Sync {
    async fn load_all(&self) -> Result<Vec<IndexProfileReadModel>, IndexProfileRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<IndexProfileReadModel>, IndexProfileRepositoryError>;
}

impl From<IndexProfileRepositoryError> for ProjectionError {
    fn from(value: IndexProfileRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
