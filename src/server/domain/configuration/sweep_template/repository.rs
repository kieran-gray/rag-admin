use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::read_model::SweepTemplateReadModel;

#[derive(Debug, Error)]
pub enum SweepTemplateRepositoryError {
    #[error("sweep template repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait SweepTemplateRepository: Send + Sync {
    async fn load_all(&self) -> Result<Vec<SweepTemplateReadModel>, SweepTemplateRepositoryError>;

    async fn save(
        &self,
        read_model: SweepTemplateReadModel,
    ) -> Result<(), SweepTemplateRepositoryError>;

    async fn set_default(&self, id: Uuid) -> Result<(), SweepTemplateRepositoryError>;

    async fn delete(&self, id: Uuid) -> Result<(), SweepTemplateRepositoryError>;
}
