use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;

use super::entity::RerankerModel;
use event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum RerankerModelRepositoryError {
    #[error("reranker model repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait RerankerModelRepository: CatalogRepository<RerankerModel> {
    async fn load_all(&self) -> Result<Vec<RerankerModel>, RerankerModelRepositoryError>;

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<RerankerModel>, RerankerModelRepositoryError>;
}

impl From<RerankerModelRepositoryError> for ProjectionError {
    fn from(value: RerankerModelRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
