use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;

use super::entity::GenerationModel;
use event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum GenerationModelRepositoryError {
    #[error("generation model repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait GenerationModelRepository: CatalogRepository<GenerationModel> {
    async fn load_all(&self) -> Result<Vec<GenerationModel>, GenerationModelRepositoryError>;

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<GenerationModel>, GenerationModelRepositoryError>;
}

impl From<GenerationModelRepositoryError> for ProjectionError {
    fn from(value: GenerationModelRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
